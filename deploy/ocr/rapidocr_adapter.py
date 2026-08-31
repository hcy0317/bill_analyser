#!/usr/bin/env python3
"""Bounded JSON adapter for the bundled RapidOCR PP-OCRv6 small runtime."""

from __future__ import annotations

import hashlib
import json
import os
from contextlib import redirect_stdout
from importlib.resources import files
from pathlib import Path
import sys
import time
import warnings

from PIL import Image
from rapidocr import (
    EngineType,
    LangCls,
    LangDet,
    LangRec,
    ModelType,
    OCRVersion,
    RapidOCR,
)


MODEL_NAME = "RapidOCR 3.9.2 / PP-OCRv6 small (ONNX Runtime)"
MODEL_HASHES = {
    "PP-OCRv6_det_small.onnx": "090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f",
    "PP-OCRv6_rec_small.onnx": "6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884",
    "ch_ppocr_mobile_v2.0_cls_mobile.onnx": "e47acedf663230f8863ff1ab0e64dd2d82b838fceb5957146dab185a89d6215c",
}
MAX_IMAGE_BYTES = int(os.getenv("BILL_ANALYSER_RAPIDOCR_MAX_IMAGE_BYTES", str(15 * 1024 * 1024)))
MAX_IMAGE_PIXELS = int(os.getenv("BILL_ANALYSER_RAPIDOCR_MAX_IMAGE_PIXELS", "25000000"))


def _model_directory() -> Path:
    return Path(str(files("rapidocr").joinpath("models")))


def _verify_models(model_dir: Path) -> None:
    for name, expected_sha256 in MODEL_HASHES.items():
        model_path = model_dir / name
        if not model_path.is_file():
            raise RuntimeError("bundled OCR model is missing")
        digest = hashlib.sha256(model_path.read_bytes()).hexdigest()
        if digest != expected_sha256:
            raise RuntimeError("bundled OCR model checksum mismatch")


def _build_engine(model_dir: Path) -> RapidOCR:
    with redirect_stdout(sys.stderr):
        return RapidOCR(
            params={
                "Global.model_root_dir": str(model_dir),
                "Det.engine_type": EngineType.ONNXRUNTIME,
                "Det.lang_type": LangDet.CH,
                "Det.model_type": ModelType.SMALL,
                "Det.ocr_version": OCRVersion.PPOCRV6,
                "Det.model_path": str(model_dir / "PP-OCRv6_det_small.onnx"),
                "Cls.engine_type": EngineType.ONNXRUNTIME,
                "Cls.lang_type": LangCls.CH,
                "Cls.model_type": ModelType.MOBILE,
                "Cls.ocr_version": OCRVersion.PPOCRV4,
                "Cls.model_path": str(model_dir / "ch_ppocr_mobile_v2.0_cls_mobile.onnx"),
                "Rec.engine_type": EngineType.ONNXRUNTIME,
                "Rec.lang_type": LangRec.CH,
                "Rec.model_type": ModelType.SMALL,
                "Rec.ocr_version": OCRVersion.PPOCRV6,
                "Rec.model_path": str(model_dir / "PP-OCRv6_rec_small.onnx"),
            }
        )


def _validate_image(path: Path) -> None:
    if not path.is_file() or path.stat().st_size <= 0 or path.stat().st_size > MAX_IMAGE_BYTES:
        raise ValueError("invalid image size")
    Image.MAX_IMAGE_PIXELS = MAX_IMAGE_PIXELS
    with warnings.catch_warnings():
        warnings.simplefilter("error", Image.DecompressionBombWarning)
        with Image.open(path) as image:
            if image.width <= 0 or image.height <= 0 or image.width * image.height > MAX_IMAGE_PIXELS:
                raise ValueError("invalid image dimensions")
            if image.format not in {"PNG", "JPEG", "WEBP", "BMP"}:
                raise ValueError("unsupported image format")
            image.verify()


def _normalize_result(result: object, elapsed_ms: int) -> dict[str, object]:
    texts_value = getattr(result, "txts", None)
    scores_value = getattr(result, "scores", None)
    boxes_value = getattr(result, "boxes", None)
    texts = tuple(texts_value) if texts_value is not None else ()
    scores = tuple(scores_value) if scores_value is not None else ()
    boxes = tuple(boxes_value) if boxes_value is not None else ()
    lines: list[dict[str, object]] = []
    for index, raw_text in enumerate(texts):
        text = str(raw_text).strip()
        if not text:
            continue
        score = float(scores[index]) if index < len(scores) else None
        box = boxes[index].tolist() if index < len(boxes) and hasattr(boxes[index], "tolist") else None
        lines.append({"text": text, "confidence": score, "bbox": box})
    if not lines:
        raise RuntimeError("bundled OCR returned no text")
    confidence_values = [line["confidence"] for line in lines if isinstance(line["confidence"], float)]
    confidence = sum(confidence_values) / len(confidence_values) if confidence_values else 0.0
    return {
        "text": "\n".join(str(line["text"]) for line in lines),
        "confidence": max(0.0, min(1.0, confidence)),
        "model": MODEL_NAME,
        "elapsed_ms": elapsed_ms,
        "lines": lines,
    }


def _check() -> int:
    model_dir = _model_directory()
    _verify_models(model_dir)
    _build_engine(model_dir)
    print(json.dumps({"ready": True, "model": MODEL_NAME}, ensure_ascii=False))
    return 0


def _recognize(image_path: str) -> int:
    path = Path(image_path)
    _validate_image(path)
    model_dir = _model_directory()
    _verify_models(model_dir)
    engine = _build_engine(model_dir)
    started_at = time.perf_counter()
    with redirect_stdout(sys.stderr):
        result = engine(str(path))
    elapsed_ms = round((time.perf_counter() - started_at) * 1000)
    print(json.dumps(_normalize_result(result, elapsed_ms), ensure_ascii=False, separators=(",", ":")))
    return 0


def main() -> int:
    if len(sys.argv) == 2 and sys.argv[1] == "--check":
        try:
            return _check()
        except Exception as error:
            print(f"bundled OCR model check failed: {type(error).__name__}: {error}", file=sys.stderr)
            return 3
    try:
        if len(sys.argv) < 2:
            raise ValueError("image path is required")
        return _recognize(sys.argv[1])
    except (ValueError, Image.UnidentifiedImageError, Image.DecompressionBombError) as error:
        if os.getenv("BILL_ANALYSER_RAPIDOCR_DEBUG") == "1":
            print(f"bundled OCR rejected the image: {type(error).__name__}: {error}", file=sys.stderr)
        else:
            print("bundled OCR rejected the image", file=sys.stderr)
        return 2
    except Exception as error:
        if os.getenv("BILL_ANALYSER_RAPIDOCR_DEBUG") == "1":
            print(f"bundled OCR model unavailable: {type(error).__name__}: {error}", file=sys.stderr)
        else:
            print("bundled OCR model unavailable", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main())
