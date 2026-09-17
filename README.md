# scopeguard (H#)

Odpowiednik rustowej biblioteki [`scopeguard`](https://github.com/bluss/scopeguard) dla języka **H#**, dystrybuowany jako pakiet **bytes.io**.

Rdzeń biblioteki (`src/scopeguard.h#`) jest napisany **w 100% w H#** — bez shimów, bez emulacji przez inny język. Jedyne miejsce z prawdziwym, systemowym FFI to `src/native.h#`, które korzysta z **prawdziwego** systemu `extern static [c]` / `extern static [rust, "..."]` H# (zero "udawanego" FFI):

- `extern static [c]` → prawdziwe symbole libc (`_exit`, `getpid`),
- `extern static [rust, "scopeguard_native"]` → prawdziwa, osobno kompilowana statyczna biblioteka Rust (`native/scopeguard_native/`), linkowana przez `--whole-archive` dokładnie tak, jak robi to kompilator H# dla `extern static [rust, ...]`.

## Instalacja (bytes.io)

W `bytes.hk` swojego projektu:

```
[deps]
-> scopeguard => https://github.com/Bytes-Repository/scopeguard.git
```

albo lokalnie, jeśli pracujesz na tym repo bezpośrednio:

```
[build]
-> include => vendor/scopeguard/src
```

i w kodzie:

```h#
use "scopeguard" from "sg"
;; albo, jeśli include wskazuje na src/:
mod scopeguard
```

## Dlaczego to nie jest dosłowny port 1:1

Rustowy `scopeguard::guard(v, f)` działa dzięki `Drop` — kompilator Rusta **sam** wstawia wywołanie `f` w każdym miejscu, w którym `_guard` traci zasięg (`return`, `?`, panika z odwijaniem stosu, koniec bloku).

H# (v0.8) **nie ma**:

1. **automatycznych destruktorów / RAII** — struktury to typy wartościowe bez hooka na "koniec życia" (zobacz `std -> sync`: `lock`/`unlock` zwracają *nowy* `Mutex`, zamiast mutować istniejący — to sposób, w jaki cała stdlib H# radzi sobie z brakiem RAII),
2. **odwijania stosu przy panice** — `panic()` w H# kończy cały proces (zobacz kompilator: `hsh_panic`), więc nie istnieje odpowiednik `Strategy::OnUnwind` — nie ma czego "łapać", bo proces już nie żyje.

Zamiast więc udawać nieistniejące RAII (strażnik, który *czasem* się nie odpala, jest gorszy niż jego brak), biblioteka daje dwa uczciwe poziomy API:

### A. `defer` / `defer_on_success` / `defer_on_error` (zalecane)

Gwarantowane wykonanie oparte na tym, że domknięcie w H# ma **własny** zasięg `return`:

```h#
mod scopeguard

fn read_and_close(path: string) -> string is
    return scopeguard::defer(
        fn() -> any is
            let f: any = fs_open(path)
            if !f is
                return ""          ;; wczesny return — kończy TYLKO `body`
            end
            return fs_read_all(f)
        end,
        fn() is
            write("sprzątanie: zawsze się wykona")
        end
    )
end
```

`return` wewnątrz `body` kończy tylko `body` (to osobne domknięcie) — `cleanup()` w `defer` i tak się wykona zaraz po nim. To dokładnie odpowiada rustowej gwarancji "na każdej ścieżce wyjścia z tego bloku".

`defer_on_success<T>` i `defer_on_error<T>` odpowiadają `guard_on_success`/duchowi `guard_on_unwind` z Rusta, ale operują na idiomatycznej dla H# konwencji `T?`/`nil` (zamiast `Result<T, E>`, którego H# nie ma):

```h#
let ok: int? = scopeguard::defer_on_success<int>(
    fn() -> int? is do_the_thing() end,
    fn() is commit_transaction() end     ;; tylko gdy wynik != nil
)

let ok2: int? = scopeguard::defer_on_error<int>(
    fn() -> int? is do_the_thing() end,
    fn() is rollback_transaction() end   ;; tylko gdy wynik == nil
)
```

### B. `Guard` / `ValueGuard` (ręczne, dla wielu punktów wyjścia)

Jawny odpowiednik `ScopeGuard`/`dismiss()`. **Ty** wywołujesz `release`/`release_value` na każdej ścieżce wyjścia — kompilator tego nie zrobi automatycznie:

```h#
let mut g: scopeguard::ValueGuard = scopeguard::guard(fd)

if error_condition is
    g = scopeguard::release_value(g, fn(handle: any) is close(handle) end)
    return
end

;; ... dalsza praca ...
g = scopeguard::release_value(g, fn(handle: any) is close(handle) end)
```

`release`/`release_value` są idempotentne — wywołanie ich drugi raz na już rozbrojonym uchwycie jest bezpiecznym no-opem, nie podwójnym sprzątaniem.

## Warstwa natywna (`extern static [c]` / `extern static [rust]`)

`src/native.h#` dokłada dwie rzeczy, których H# nie ma jako część języka:

1. **`emergency_exit(cleanup, code)`** (przez `defer_exit` w głównym module) — uruchamia `cleanup`, po czym kończy proces prawdziwym `_exit(2)` z libc. To odpowiednik ostrzeżenia z dokumentacji Rusta: `std::process::exit` pomija `Drop` — tutaj `defer_exit` explicite gwarantuje wykonanie sprzątania *przed* wyjściem.
2. **`next_id()`** — globalnie unikalny w ramach procesu, wątkowo-bezpieczny licznik (`AtomicI64` po stronie Rusta) do znakowania/logowania wielu aktywnych strażników. `std -> sync` w H# ma tylko nazwane liczniki (`atomic_add(name, val)`), nie atomik jako wartość pierwszej klasy — stąd realny (nie kosmetyczny) powód sięgnięcia po Rusta.

Zbudowanie warstwy natywnej (opcjonalne — reszta biblioteki działa bez niej):

```sh
cd native/scopeguard_native
cargo build --release
# skopiuj wynikowy staticlib tam, gdzie linker H# go znajdzie
cp target/release/libscopeguard_native.a ../../
```

Jeśli `libscopeguard_native.a` nie zostanie zlinkowany, funkcje korzystające z `extern static [rust, "scopeguard_native"]` (`next_id`, `native_version`) zawiodą na etapie linkowania — dokładnie tak samo, jak przy brakującej bibliotece C, bez żadnego "cichego" fallbacku.

## Struktura pakietu

```
scopeguard/
├── bytes.hk                          ; manifest bytes.io
├── src/
│   ├── scopeguard.h#                 ; rdzeń — 100% H#
│   └── native.h#                     ; extern static [c] / [rust]
├── native/scopeguard_native/         ; prawdziwy crate Rust (staticlib)
│   ├── Cargo.toml
│   └── src/lib.rs
├── tests/scopeguard_test.h#
└── examples/
    ├── basic.h#
    ├── manual_guard.h#
    └── native_ffi.h#
```

## Testy

```sh
bytes test
```

## Licencja

MIT — zobacz `LICENSE`. Oryginalna biblioteka `scopeguard` (Rust) jest na licencji MIT/Apache-2.0: https://github.com/bluss/scopeguard.
