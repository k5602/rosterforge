#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz that patch_data always produces valid output when given valid input,
// or returns an error without panicking. The invariant is:
// validate_data(user_data) == Ok(t3db) implies
// validate_data(patch_data(user_data, database)) is Ok or Err
// (never panics, and if database starts with T3DB and patching succeeds,
// the result must validate).
fuzz_target!(|data: &[u8]| {
    let _ = rf::save_format::find_t3db(data);
    if let Ok(_) = rf::save_format::validate_data(data) {
        // Try patching with various "databases" - some valid, some not.
        // Just ensure no panics; correctness is invariant-tested elsewhere.
        let _ = rf::patch::patch_data(data, b"invalid_database");
        let db = [rf::save_format::T3DB.as_slice(), b"somedata"].concat();
        let _ = rf::patch::patch_data(data, &db);
    }
});
