use std::sync::atomic::{AtomicI64, Ordering};

/// Wersja ABI tej biblioteki natywnej — bump'owana przy KAŻDEJ zmianie
/// sygnatury eksportowanej funkcji, żeby `native::native_abi_version()`
/// po stronie H# mogło to zweryfikować w runtime.
const NATIVE_ABI_VERSION: i64 = 1;

static GUARD_ID_COUNTER: AtomicI64 = AtomicI64::new(0);

/// Kolejny, globalnie unikalny (w ramach procesu) identyfikator strażnika.
/// Odpowiada `int` po stronie H# (H#'s `int` to i64 — patrz przykłady w
/// showcase.h#: `let d: i64 = 9_223_372_036_854_775_807`).
#[no_mangle]
pub extern "C" fn sg_next_id() -> i64 {
    // fetch_add zwraca poprzednią wartość, więc pierwsze wywołanie da 0,
    // tak jak liczyłby to każdy zwykły licznik od zera.
    GUARD_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Wersja ABI tej biblioteki natywnej.
#[no_mangle]
pub extern "C" fn sg_native_abi_version() -> i64 {
    NATIVE_ABI_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonic_and_unique() {
        let a = sg_next_id();
        let b = sg_next_id();
        let c = sg_next_id();
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn abi_version_is_stable() {
        assert_eq!(sg_native_abi_version(), 1);
    }
}
