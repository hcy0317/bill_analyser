#[cfg(test)]
mod tests {
    use super::*;

    fn user_id(value: u64) -> UserId {
        UserId::new(value).expect("positive user id")
    }

    include!("auth/token_sessions.rs");
    include!("auth/profile_cloud.rs");
    include!("auth/two_factor.rs");
    include!("auth/login_failure.rs");
}
