"""Neutral category adapter."""

from __future__ import annotations

from typing import Any

from bill_analyser.utils.constants import DEFAULT_PARENT_ID


class CategoryAdapter:
    """分类数据适配器。"""

    def backend_to_frontend(self, category: dict[str, Any], parent_id: str = DEFAULT_PARENT_ID) -> dict[str, Any]:
        """后端分类 -> 前端分类。"""
        name = category.get("sub_category") or category.get("main_category", "")
        return {
            "id": str(category.get("id", "")),
            "name": name,
            "parentId": str(parent_id),
            "type": category.get("type", 0),
            "icon": category.get("icon", ""),
            "color": category.get("color", ""),
            "comment": category.get("description", ""),
            "displayOrder": category.get("priority", 0),
            "hidden": bool(category.get("hidden", False)),
            "visible": not bool(category.get("hidden", False)),
            "keywords": category.get("keywords", ""),
        }

    def format_list_response(self, categories: list[dict[str, Any]]) -> dict[str, Any]:
        """按类型构建分类树。"""
        grouped: dict[int, list[dict[str, Any]]] = {}
        main_nodes: dict[tuple, dict[str, Any]] = {}

        for category in categories:
            cat_type = int(category.get("type", 0) or 0)
            grouped.setdefault(cat_type, [])

            main_name = category.get("main_category", "")
            sub_name = category.get("sub_category", "")
            key = (cat_type, main_name)

            if key not in main_nodes:
                virtual_id = f"virtual_{main_name}"
                main_nodes[key] = {
                    "id": str(category.get("id", virtual_id)) if not sub_name else virtual_id,
                    "name": main_name,
                    "parentId": DEFAULT_PARENT_ID,
                    "type": cat_type,
                    "icon": category.get("icon", ""),
                    "color": category.get("color", ""),
                    "comment": "",
                    "displayOrder": category.get("priority", 0),
                    "hidden": False,
                    "visible": True,
                    "keywords": "",
                    "subCategories": [],
                }
                grouped[cat_type].append(main_nodes[key])

            node = main_nodes[key]
            if not sub_name:
                node.update(self.backend_to_frontend(category))
                node["parentId"] = DEFAULT_PARENT_ID
                node.setdefault("subCategories", [])
            else:
                node.setdefault("subCategories", []).append(self.backend_to_frontend(category, parent_id=node["id"]))

        return {"success": True, "result": grouped}

    def get_flat_list(self, categories: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """返回扁平分类列表。"""
        flat_list: list[dict[str, Any]] = []
        for category in categories:
            parent_id = (
                DEFAULT_PARENT_ID
                if not category.get("sub_category")
                else f"virtual_{category.get('main_category', '')}"
            )
            flat_list.append(self.backend_to_frontend(category, parent_id=parent_id))
        return flat_list
