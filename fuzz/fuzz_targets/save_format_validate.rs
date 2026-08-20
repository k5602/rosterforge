#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = rf::save_format::find_t3db(data);
    let _ = rf::save_format::validate_data(data);
    let _ = rf::save_format::database_from_generated_squads(data);
    if let Ok(name) = rf::save_format::save_name(data) {
        let _ = rf::save_format::safe_folder_name(&name);
    }
});
