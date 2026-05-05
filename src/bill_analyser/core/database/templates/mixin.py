"""Template database mixin facade."""

# pylint: disable=too-few-public-methods

from __future__ import annotations

from bill_analyser.core.database.shared import DatabaseFacadeBase

from .crud import (
    _build_template_update_payload,
    _get_next_template_display_order,
    _get_template_table,
    create_template,
    delete_template,
    get_all_templates,
    get_template_by_id,
    update_template,
    update_template_display_orders,
)
from .recurring import (
    _recalculate_recurring_next_date,
    bind_bill_to_recurring,
    build_recurring_candidates_for_bill_data,
    get_enabled_recurring_templates,
    get_recurring_candidates_for_bill,
    unbind_bill_from_recurring,
)
from .schedule import (
    _find_recurring_occurrence_near_date,
    _get_first_recurring_occurrence,
    _get_next_recurring_occurrence_after,
    _is_recurring_active_on_date,
    _is_recurring_due_on_date,
    _parse_date_value,
    _parse_schedule_frequency_values,
    _weekday_sunday_first,
)
from .serialization import (
    _deserialize_template_tag_ids,
    _normalize_template_transaction_type,
    _serialize_template_row,
    _serialize_template_tag_ids,
)


class DatabaseTemplatesMixin(DatabaseFacadeBase):
    """Template and recurring-bill persistence helpers."""

    get_all_templates = get_all_templates
    get_template_by_id = get_template_by_id
    create_template = create_template
    update_template = update_template
    delete_template = delete_template
    update_template_display_orders = update_template_display_orders
    get_recurring_candidates_for_bill = get_recurring_candidates_for_bill
    get_enabled_recurring_templates = get_enabled_recurring_templates
    build_recurring_candidates_for_bill_data = build_recurring_candidates_for_bill_data
    bind_bill_to_recurring = bind_bill_to_recurring
    _recalculate_recurring_next_date = _recalculate_recurring_next_date
    unbind_bill_from_recurring = unbind_bill_from_recurring
    _get_first_recurring_occurrence = _get_first_recurring_occurrence
    _get_template_table = _get_template_table
    _get_next_template_display_order = _get_next_template_display_order
    _serialize_template_tag_ids = _serialize_template_tag_ids
    _deserialize_template_tag_ids = _deserialize_template_tag_ids
    _serialize_template_row = _serialize_template_row
    _normalize_template_transaction_type = _normalize_template_transaction_type
    _parse_date_value = _parse_date_value
    _parse_schedule_frequency_values = _parse_schedule_frequency_values
    _weekday_sunday_first = _weekday_sunday_first
    _is_recurring_active_on_date = _is_recurring_active_on_date
    _is_recurring_due_on_date = _is_recurring_due_on_date
    _find_recurring_occurrence_near_date = _find_recurring_occurrence_near_date
    _get_next_recurring_occurrence_after = _get_next_recurring_occurrence_after
    _build_template_update_payload = _build_template_update_payload
