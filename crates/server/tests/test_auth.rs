//! Authentication tests for the application.

// lib imports
use chrono::{Duration, Utc};
use rstest::rstest;

// local imports
use koko::auth::{
    AdminGuard,
    AuthGuard,
    UserGuard,
    create_token,
    decode_token,
    hash_password,
    verify_password,
};

#[rstest]
#[case("123", "user with numeric ID")]
#[case("admin", "user with text ID")]
#[case("user@domain.com", "user with email-like ID")]
#[case("user_with_underscores", "user with underscores")]
fn test_jwt_token_creation_and_verification(
    #[case] user_id: &str,
    #[case] _description: &str,
) {
    let secret = "test_secret_key_for_jwt";

    // Test token creation
    let token = create_token(user_id, secret).expect("Should create token");
    assert!(!token.is_empty());

    // Test token decoding
    let claims = decode_token(&token, secret).expect("Should decode token");
    assert_eq!(claims.sub, user_id);

    // Verify expiration is in the future (24 hours)
    let now = Utc::now().timestamp() as usize;
    assert!(claims.exp > now);

    // Should be approximately 24 hours from now (allowing 1 minute tolerance)
    let expected_exp = (Utc::now() + Duration::hours(24)).timestamp() as usize;
    assert!((claims.exp as i64 - expected_exp as i64).abs() < 60);
}

#[rstest]
#[case("test_secret_key", "wrong_secret_key")]
#[case("short", "different")]
#[case(
    "very_long_secret_key_123456789",
    "another_very_long_secret_key_987654321"
)]
fn test_jwt_token_with_invalid_secret(
    #[case] correct_secret: &str,
    #[case] wrong_secret: &str,
) {
    let user_id = "test_user";

    let token = create_token(user_id, correct_secret).expect("Should create token");

    // Should fail with wrong secret
    let result = decode_token(&token, wrong_secret);
    assert!(result.is_err());
}

#[rstest]
#[case(
    "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMiLCJleHAiOjF9.invalid",
    "obviously expired token"
)]
#[case("invalid.token.format", "malformed token")]
#[case("", "empty token")]
fn test_jwt_token_invalid_cases(
    #[case] invalid_token: &str,
    #[case] _description: &str,
) {
    let secret = "test_secret";

    let result = decode_token(invalid_token, secret);
    assert!(result.is_err());
}

#[test]
fn test_password_hashing_with_bcrypt() {
    let password = "test_password_123";

    // Test hashing
    let hash = hash_password(password).expect("Should hash password");
    assert!(!hash.is_empty());
    assert_ne!(hash, password); // Hash should be different from password

    // Test verification with correct password
    assert!(verify_password(password, &hash));

    // Test verification with wrong password
    assert!(!verify_password("wrong_password", &hash));
}

#[test]
fn test_same_password_different_hashes() {
    let password = "same_password";

    let hash1 = hash_password(password).expect("Should hash password");
    let hash2 = hash_password(password).expect("Should hash password");

    // BCrypt should produce different hashes even for the same password due to internal salting
    assert_ne!(hash1, hash2);

    // But both should verify correctly
    assert!(verify_password(password, &hash1));
    assert!(verify_password(password, &hash2));
}

#[rstest]
#[case("", "empty password")]
#[case(&"a".repeat(1000), "very long password")]
#[case("!@#$%^&*()_+-=[]{}|;':\",./<>?`~", "password with special characters")]
#[case("simple123", "simple alphanumeric password")]
#[case("Κωδικός123", "password with unicode characters")]
fn test_password_hashing_edge_cases(
    #[case] password: &str,
    #[case] _description: &str,
) {
    // Test hashing
    let hash = hash_password(password).expect("Should hash password");
    assert!(!hash.is_empty());
    assert_ne!(hash, password); // Hash should be different from password

    // Test verification with correct password
    assert!(verify_password(password, &hash));

    // Test verification with wrong password (unless password is empty)
    if !password.is_empty() {
        assert!(!verify_password("different_password", &hash));
    }
}

#[rstest]
#[case("password123")]
#[case("another_password")]
#[case("complex!Pass@123")]
fn test_bcrypt_salt_uniqueness(#[case] password: &str) {
    // Hash the same password multiple times
    let hash1 = hash_password(password).expect("Should hash password");
    let hash2 = hash_password(password).expect("Should hash password");

    // BCrypt should produce different hashes even with same input due to internal salt
    assert_ne!(hash1, hash2);

    // But both should verify correctly
    assert!(verify_password(password, &hash1));
    assert!(verify_password(password, &hash2));
}

#[test]
fn test_auth_guard_role_constants() {
    // Test valid role constants
    assert_eq!(UserGuard::role(), koko::auth::Role::User);
    assert_eq!(AdminGuard::role(), koko::auth::Role::Admin);
}

#[test]
fn test_claims_struct_functionality() {
    use koko::auth::Claims;

    // Test that we can create and access Claims - this exercises the public API
    // that AuthGuard.claims() would return
    let test_claims = Claims {
        sub: "test_user_123".to_string(),
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    };

    // Verify the claims data is accessible (same as what .claims() would return)
    assert_eq!(test_claims.sub, "test_user_123");
    assert!(test_claims.exp > Utc::now().timestamp() as usize);

    // Test that Claims can be cloned (important for the claims() method return value usage)
    let cloned_claims = test_claims.clone();
    assert_eq!(cloned_claims.sub, test_claims.sub);
    assert_eq!(cloned_claims.exp, test_claims.exp);
}

#[test]
#[should_panic(expected = "Invalid role constant")]
fn test_auth_guard_invalid_role_constant() {
    // This should panic because role constant 99 is not valid
    // We test the panic by calling the role() method with an invalid const generic
    let _ = AuthGuard::<99>::role();
}
