#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(value: u64) -> UserId {
        UserId::new(value).expect("positive user id")
    }

    include!("tests/token_sessions.rs");
    include!("tests/profile_cloud.rs");
    include!("tests/two_factor.rs");
    include!("tests/login_failure.rs");
}
