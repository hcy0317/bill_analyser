pub const DEFAULT_PARENT_ID: &str = "0";

pub fn virtual_parent_id(main_category: &str) -> String {
    format!("virtual_{}", main_category)
}
