# Bundled OCR third-party notice

Bill Analyser's optional bundled local OCR runtime installs these pinned components while building the backend image:

- RapidOCR 3.9.2 — Apache-2.0 — https://github.com/RapidAI/RapidOCR
- PaddleOCR PP-OCRv6 small detection and recognition models — model copyright Baidu, distributed by RapidOCR under the upstream Apache-2.0 notice — https://github.com/PaddlePaddle/PaddleOCR
- ONNX Runtime 1.29.0 — MIT — https://github.com/microsoft/onnxruntime
- OpenCV Python headless 5.0.0.93 — Apache-2.0 — https://github.com/opencv/opencv-python

RapidOCR 3.9.2 packages these model files so the production runtime does not download models on first use:

- `PP-OCRv6_det_small.onnx` — SHA256 `090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f`
- `PP-OCRv6_rec_small.onnx` — SHA256 `6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884`
- `ch_ppocr_mobile_v2.0_cls_mobile.onnx` — SHA256 `e47acedf663230f8863ff1ab0e64dd2d82b838fceb5957146dab185a89d6215c`

The adapter verifies these hashes before initializing the model. Receipt images remain on the Bill Analyser server when this provider is selected.
