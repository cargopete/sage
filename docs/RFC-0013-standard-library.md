# RFC-0013: Standard Library

- **Status:** Draft
- **Created:** 2026-03-13
- **Author:** Pete Pavlovski
- **Depends on:** RFC-0003 (Compile to Rust), RFC-0005 (User-Defined Types), RFC-0009 (First-Class Functions), RFC-0010 (Maps, Tuples, Result)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Feature 1 — String Functions](#4-feature-1--string-functions)
5. [Feature 2 — List Higher-Order Functions](#5-feature-2--list-higher-order-functions)
6. [Feature 3 — Math Functions](#6-feature-3--math-functions)
7. [Feature 4 — Parsing and Conversion](#7-feature-4--parsing-and-conversion)
8. [Feature 5 — Modulo Operator `%`](#8-feature-5--modulo-operator-)
9. [Feature 6 — String Interpolation Improvements](#9-feature-6--string-interpolation-improvements)
10. [Feature 7 — `sage new` CLI Command](#10-feature-7--sage-new-cli-command)
11. [Checker Rules](#11-checker-rules)
12. [Codegen](#12-codegen)
13. [New Error Codes](#13-new-error-codes)
14. [Implementation Plan](#14-implementation-plan)
15. [Open Questions](#15-open-questions)

---

## 1. Summary

This RFC introduces the Sage standard library — a set of builtin functions and language improvements that close the most persistent day-to-day gaps in the language. Without these, writing practical programs requires awkward workarounds: you cannot split a string, filter a list without a manual loop, compute absolute values, or parse an integer from user input.

The seven features in this RFC are:

- **String functions** — `split`, `trim`, `starts_with`, `ends_with`, `replace`, `to_upper`, `to_lower`, `str_len`, `str_slice`, `str_index_of`
- **List higher-order functions** — `map`, `filter`, `reduce`, `any`, `all`, `find`, `flat_map`, `zip`, `sort_by`
- **Math functions** — `abs`, `min`, `max`, `floor`, `ceil`, `round`, `pow`, `sqrt`, `clamp`
- **Parsing and conversion** — `parse_int`, `parse_float`, `parse_bool`
- **Modulo operator `%`** — integer and float remainder
- **String interpolation improvements** — field access (`{person.name}`), method-style calls (`{list.len()}`), and indexed access (`{items.0}`) inside interpolated strings
- **`sage new` CLI command** — scaffold a new Sage project on disk

These features are all implemented as additions to `sage-runtime` and `sage-checker`. No existing code changes behaviour. This is a pure additive RFC.

---

## 2. Motivation

### What you cannot do today without workarounds

The current builtin set (`print`, `len`, `push`, `join`, `str`, `int_to_str`, `str_contains`, `sleep_ms`) was sufficient to demonstrate the language but is inadequate for real programs. Here is a sample of things that are currently impossible or require painful manual loops:

**Splitting a string by a delimiter:**
```sage
// Wanted:
let words = split(sentence, " ")

// Reality: no split function exists. You must use infer() with an LLM
// to split a string, which is absurd.
```

**Filtering a list:**
```sage
// Wanted:
let long_words = filter(words, fn(w: String) -> Bool { return len(w) > 5 })

// Reality: manual for loop with a push to a new list
let long_words: List<String> = []
for word in words {
    if str_contains(word, "...") {  // still can't check length easily
        long_words = push(long_words, word)
    }
}
```

**Parsing a string as a number:**
```sage
// Wanted:
let n = parse_int("42")  // -> Result<Int, String>

// Reality: impossible. If an LLM returns "42" as a string, you cannot
// convert it to an Int for arithmetic.
```

**Computing a remainder:**
```sage
// Wanted:
let is_even = n % 2 == 0

// Reality: % operator does not exist.
// Workaround: n - (n / 2) * 2  (only works for positive integers)
```

**String interpolation with field access:**
```sage
// Wanted:
let msg = "Hello, {person.name}! You are {person.age} years old."

// Reality: must extract to locals first:
let name = person.name
let age = person.age
let msg = "Hello, {name}! You are {age} years old."
```

Each of these is a friction point that makes Sage feel unfinished. Developers evaluating the language hit these walls quickly and conclude the language is not ready. This RFC addresses all of them.

### Why bundled builtins rather than a library

The language already has special-cased builtins (`len`, `push`, `join`) implemented directly in the checker and codegen. The pattern is established: builtins are functions that exist in the Sage prelude with known signatures, special-cased in the checker for generic handling where needed, and emitted as direct Rust calls in codegen.

The alternative — implementing these as a `sage_stdlib` package that users import — would require user-defined generics (RFC pending), package infrastructure for stdlib packages, and would mean simple things like `split(s, ",")` require an import declaration. That is wrong for functions this fundamental. They belong in the prelude.

---

## 3. Design Goals

1. **Familiar names.** Function names follow the most common convention across Rust, Python, and JavaScript. A developer should be able to guess the name correctly on the first try.
2. **Consistent signatures.** All string functions take the string as the first argument. All list HOFs take the list as the first argument and the function as the second. No surprises about parameter ordering.
3. **Fallible parsing returns `Result`.** `parse_int`, `parse_float`, and `parse_bool` return `Result<T, String>`, integrating with `try`/`catch`. They do not panic.
4. **HOFs use RFC-0009 closures.** `map`, `filter`, and friends take `Fn(T) -> U` parameters, exactly as established in RFC-0009.
5. **Modulo follows Rust semantics.** `%` on integers is the remainder (not modulo — sign follows the dividend). This is what developers expect from a C-family language.
6. **String interpolation improvements are backward-compatible.** Existing `{ident}` interpolations continue to work unchanged. New syntax (`{person.name}`) is a strict superset.
7. **`sage new` is simple and opinionated.** It scaffolds a minimal project. No options, no templates, no wizard. Fast.

---

## 4. Feature 1 — String Functions

### 4.1 Full function list

All string functions are pure and infallible unless noted. They are added to the Sage prelude and require no import.

| Function | Signature | Description |
|----------|-----------|-------------|
| `split` | `(String, String) -> List<String>` | Split string on delimiter. Returns `[""]` for empty string. Trailing empty strings are included. |
| `trim` | `(String) -> String` | Remove leading and trailing whitespace (space, tab, newline, carriage return). |
| `trim_start` | `(String) -> String` | Remove leading whitespace only. |
| `trim_end` | `(String) -> String` | Remove trailing whitespace only. |
| `starts_with` | `(String, String) -> Bool` | True if string begins with prefix. Empty prefix always returns true. |
| `ends_with` | `(String, String) -> Bool` | True if string ends with suffix. |
| `replace` | `(String, String, String) -> String` | Replace all occurrences of the second argument with the third. |
| `replace_first` | `(String, String, String) -> String` | Replace only the first occurrence. |
| `to_upper` | `(String) -> String` | Convert all ASCII letters to uppercase. Unicode letters are passed through unchanged in this release (see §15.1). |
| `to_lower` | `(String) -> String` | Convert all ASCII letters to lowercase. |
| `str_len` | `(String) -> Int` | Length in Unicode code points (not bytes). |
| `str_slice` | `(String, Int, Int) -> String` | Substring by code point indices `[start, end)`. Negative indices are not supported. Out-of-bounds returns empty string (not an error). |
| `str_index_of` | `(String, String) -> Option<Int>` | Returns the code point index of the first occurrence of the needle, or `None`. |
| `str_repeat` | `(String, Int) -> String` | Repeat string N times. Returns empty string for N ≤ 0. |
| `str_pad_start` | `(String, Int, String) -> String` | Pad string on the left to reach length N using the pad character. |
| `str_pad_end` | `(String, Int, String) -> String` | Pad string on the right. |

### 4.2 Usage examples

```sage
let words = split("hello world foo", " ")
// ["hello", "world", "foo"]

let cleaned = trim("  hello\n  ")
// "hello"

let upper = to_upper("hello Sage")
// "HELLO SAGE"

let replaced = replace("aabbcc", "b", "X")
// "aaXXcc"

let idx = str_index_of("hello", "ll")
// Some(2)

let missing = str_index_of("hello", "xyz")
// None

let sub = str_slice("hello", 1, 4)
// "ell"

let padded = str_pad_start("42", 5, "0")
// "00042"
```

### 4.3 Relationship to existing builtins

`str_contains` already exists and is unchanged. `str_len` is new and distinct from `len` — `len` operates on `List<T>`, while `str_len` operates on `String`. The checker enforces this distinction. Calling `len` on a `String` is a type error; calling `str_len` on a `List<T>` is also a type error. Both remain separate builtins; there is no overloading.

`int_to_str` already exists. `str` (generic to-string) already exists. These are not changed.

### 4.4 Checker handling

String functions have fixed, concrete signatures and require no special-casing in the checker beyond registering their signatures in the symbol table. They are simpler than `len` or `push` (which needed generic handling).

The exception is `str_index_of`, which returns `Option<Int>`. The checker must verify that callers handle the `Option` — either via `match`, `if let`-style pattern, or by unwrapping with a fallback. This follows the existing `Option<T>` handling rules from RFC-0005.

---

## 5. Feature 2 — List Higher-Order Functions

### 5.1 The HOF gap

RFC-0009 added first-class functions and closures specifically to enable higher-order programming. The type system can express `Fn(String) -> Bool`. But there is nothing to pass these functions *to*. You cannot `filter` a list, `map` over it, or `reduce` it. The closures are a solution without a problem to solve. This feature completes the picture.

### 5.2 Full function list

| Function | Signature | Description |
|----------|-----------|-------------|
| `map` | `(List<A>, Fn(A) -> B) -> List<B>` | Apply function to each element, return new list. |
| `filter` | `(List<A>, Fn(A) -> Bool) -> List<A>` | Return elements where predicate is true. |
| `reduce` | `(List<A>, B, Fn(B, A) -> B) -> B` | Fold list left to right with initial accumulator. Returns initial value for empty list. |
| `any` | `(List<A>, Fn(A) -> Bool) -> Bool` | True if predicate is true for at least one element. False for empty list. |
| `all` | `(List<A>, Fn(A) -> Bool) -> Bool` | True if predicate is true for all elements. True for empty list. |
| `find` | `(List<A>, Fn(A) -> Bool) -> Option<A>` | Return the first element matching the predicate, or `None`. |
| `flat_map` | `(List<A>, Fn(A) -> List<B>) -> List<B>` | Map and flatten one level. |
| `zip` | `(List<A>, List<B>) -> List<(A, B)>` | Pair elements from two lists. Result length is the shorter list's length. |
| `sort_by` | `(List<A>, Fn(A, A) -> Int) -> List<A>` | Sort by comparator. Comparator must return negative (less), zero (equal), or positive (greater). Returns a new list; does not sort in place. |
| `enumerate` | `(List<A>) -> List<(Int, A)>` | Pair each element with its zero-based index. |
| `take` | `(List<A>, Int) -> List<A>` | First N elements. Returns full list if N ≥ length. Returns empty list for N ≤ 0. |
| `drop` | `(List<A>, Int) -> List<A>` | Skip first N elements. |
| `flatten` | `(List<List<A>>) -> List<A>` | Flatten one level of nesting. |
| `reverse` | `(List<A>) -> List<A>` | Return list in reversed order. |
| `unique` | `(List<A>) -> List<A>` | Remove duplicates, preserving first occurrence. Requires `A` to be comparable (same constraint as `==`). |
| `count_where` | `(List<A>, Fn(A) -> Bool) -> Int` | Count elements matching predicate. |
| `sum` | `(List<Int>) -> Int` | Sum of all integers. Returns 0 for empty list. |
| `sum_floats` | `(List<Float>) -> Float` | Sum of all floats. Returns 0.0 for empty list. |

### 5.3 Usage examples

```sage
let nums = [1, 2, 3, 4, 5]

let doubled = map(nums, fn(n: Int) -> Int { return n * 2 })
// [2, 4, 6, 8, 10]

let evens = filter(nums, fn(n: Int) -> Bool { return n % 2 == 0 })
// [2, 4]

let total = reduce(nums, 0, fn(acc: Int, n: Int) -> Int { return acc + n })
// 15

let has_big = any(nums, fn(n: Int) -> Bool { return n > 4 })
// true

let first_even = find(nums, fn(n: Int) -> Bool { return n % 2 == 0 })
// Some(2)

let words = ["banana", "apple", "cherry"]
let sorted = sort_by(words, fn(a: String, b: String) -> Int {
    if a < b { return -1 }
    if a > b { return 1 }
    return 0
})
// ["apple", "banana", "cherry"]

let indexed = enumerate(["a", "b", "c"])
// [(0, "a"), (1, "b"), (2, "c")]

let pairs = zip([1, 2, 3], ["a", "b", "c"])
// [(1, "a"), (2, "b"), (3, "c")]
```

### 5.4 Type checker handling for generic HOFs

`map`, `filter`, `reduce`, `flat_map`, `zip`, and `find` are **polymorphic** — their return type depends on the element type of the list and the return type of the passed function. This requires special-casing in the checker, extending the pattern already used for `len` and `push`.

For `map(list, f)`:
1. Check `list` has type `List<A>` for some `A`.
2. Check `f` has type `Fn(A) -> B` for some `B`.
3. Return type is `List<B>`.

For `filter(list, f)`:
1. Check `list` has type `List<A>`.
2. Check `f` has type `Fn(A) -> Bool`.
3. Return type is `List<A>`.

For `reduce(list, init, f)`:
1. Check `list` has type `List<A>`.
2. Check `init` has type `B`.
3. Check `f` has type `Fn(B, A) -> B`.
4. Return type is `B`.

For `find(list, f)`:
1. Check `list` has type `List<A>`.
2. Check `f` has type `Fn(A) -> Bool`.
3. Return type is `Option<A>`.

For `zip(a, b)`:
1. Check `a` has type `List<A>`.
2. Check `b` has type `List<B>`.
3. Return type is `List<(A, B)>` — a list of tuples (requires RFC-0010 tuples).

For `sort_by(list, f)`:
1. Check `list` has type `List<A>`.
2. Check `f` has type `Fn(A, A) -> Int`.
3. Return type is `List<A>`.

For `enumerate(list)`:
1. Check `list` has type `List<A>`.
2. Return type is `List<(Int, A)>`.

For `unique(list)`:
1. Check `list` has type `List<A>`.
2. Check `A` is a comparable type (same constraint as using `==`).
3. Return type is `List<A>`.

`sum` and `sum_floats` have concrete signatures and require no special-casing.

### 5.5 Codegen for HOFs

HOFs generate direct calls to Rust iterator methods on `Vec<T>`:

| Sage | Generated Rust |
|------|----------------|
| `map(list, f)` | `list.into_iter().map(f).collect::<Vec<_>>()` |
| `filter(list, f)` | `list.into_iter().filter(f).collect::<Vec<_>>()` |
| `reduce(list, init, f)` | `list.into_iter().fold(init, f)` |
| `any(list, f)` | `list.iter().any(f)` |
| `all(list, f)` | `list.iter().all(f)` |
| `find(list, f)` | `list.into_iter().find(f)` |
| `flat_map(list, f)` | `list.into_iter().flat_map(f).collect::<Vec<_>>()` |
| `zip(a, b)` | `a.into_iter().zip(b.into_iter()).collect::<Vec<_>>()` |
| `enumerate(list)` | `list.into_iter().enumerate().map(\|(i, v)\| (i as i64, v)).collect::<Vec<_>>()` |
| `take(list, n)` | `list.into_iter().take(n as usize).collect::<Vec<_>>()` |
| `drop(list, n)` | `list.into_iter().skip(n as usize).collect::<Vec<_>>()` |
| `flatten(list)` | `list.into_iter().flatten().collect::<Vec<_>>()` |
| `reverse(list)` | `{ let mut v = list; v.reverse(); v }` |
| `sort_by(list, f)` | `{ let mut v = list; v.sort_by(\|a, b\| match f(a.clone(), b.clone()) { n if n < 0 => std::cmp::Ordering::Less, 0 => std::cmp::Ordering::Equal, _ => std::cmp::Ordering::Greater }); v }` |
| `unique(list)` | `{ let mut seen = std::collections::HashSet::new(); list.into_iter().filter(\|x\| seen.insert(x.clone())).collect::<Vec<_>>() }` |
| `count_where(list, f)` | `list.iter().filter(f).count() as i64` |
| `sum(list)` | `list.iter().sum::<i64>()` |
| `sum_floats(list)` | `list.iter().sum::<f64>()` |

The `into_iter()` vs `iter()` choice depends on whether the closure takes ownership. Since Sage values are cloned at call boundaries (consistent with the current codegen model for records and strings), `into_iter()` is used when the closure takes owned values, `iter()` when it only borrows.

---

## 6. Feature 3 — Math Functions

### 6.1 Full function list

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `(Int) -> Int` | Absolute value. `abs(-5)` → `5`. `abs(Int::MIN)` returns `Int::MIN` (Rust wrapping semantics). |
| `abs_float` | `(Float) -> Float` | Absolute value for floats. |
| `min` | `(Int, Int) -> Int` | Smaller of two integers. |
| `max` | `(Int, Int) -> Int` | Larger of two integers. |
| `min_float` | `(Float, Float) -> Float` | Smaller of two floats. |
| `max_float` | `(Float, Float) -> Float` | Larger of two floats. |
| `clamp` | `(Int, Int, Int) -> Int` | `clamp(value, low, high)` — constrain value to `[low, high]`. If `low > high`, returns `low`. |
| `clamp_float` | `(Float, Float, Float) -> Float` | Clamp for floats. |
| `floor` | `(Float) -> Int` | Round toward negative infinity. Returns `Int`. |
| `ceil` | `(Float) -> Int` | Round toward positive infinity. Returns `Int`. |
| `round` | `(Float) -> Int` | Round to nearest integer, half rounds away from zero. Returns `Int`. |
| `floor_float` | `(Float) -> Float` | Floor but returns `Float` (useful for intermediate calculations). |
| `ceil_float` | `(Float) -> Float` | Ceil returning `Float`. |
| `pow` | `(Int, Int) -> Int` | Integer exponentiation. `pow(2, 10)` → `1024`. Negative exponents return 0 for integers. |
| `pow_float` | `(Float, Float) -> Float` | Floating-point exponentiation. |
| `sqrt` | `(Float) -> Float` | Square root. Returns `NaN` for negative inputs (Rust f64 semantics). |
| `int_to_float` | `(Int) -> Float` | Cast integer to float. |
| `float_to_int` | `(Float) -> Int` | Truncate float to integer (round toward zero). |

### 6.2 Naming rationale: `_float` suffixes

Sage does not support function overloading. `abs(Int)` and `abs(Float)` cannot share a name. The convention `abs` for integers, `abs_float` for floats follows the precedent set by `int_to_str` (not `to_str` — explicitly typed). This is verbose but unambiguous and consistent.

An alternative would be to make these generic with type-parameterised dispatch. That requires user-defined generics (a future RFC). Until then, the `_float` suffix is the right pragmatic choice.

### 6.3 Constants

Two numeric constants are added to the prelude:

```sage
const PI: Float = 3.141592653589793
const E: Float  = 2.718281828459045
```

These are `const` declarations (per RFC-0005 §10), not functions. They are accessible everywhere without import.

### 6.4 Usage examples

```sage
let x = abs(-42)         // 42
let y = max(3, 7)        // 7
let z = clamp(15, 0, 10) // 10

let angle = PI * 2.0
let r = sqrt(2.0)        // 1.4142135623730951

let floored = floor(3.7) // 3   (Int)
let ceiled  = ceil(3.2)  // 4   (Int)
let rounded = round(3.5) // 4   (Int)
```

---

## 7. Feature 4 — Parsing and Conversion

### 7.1 The need

Agents that interact with external systems — HTTP responses, user input, LLM-extracted values — frequently receive data as strings that need to be interpreted as numbers or booleans. Without parsing functions, the only route is to use `infer()` with an LLM for type conversion, which is both expensive and unreliable.

### 7.2 Full function list

| Function | Signature | Description |
|----------|-----------|-------------|
| `parse_int` | `(String) -> Result<Int, String>` | Parse string as decimal integer. Accepts optional leading `+`/`-`. Returns `Err` for empty string, non-numeric characters, or overflow. |
| `parse_float` | `(String) -> Result<Float, String>` | Parse string as floating-point number. Accepts `"inf"`, `"-inf"`, `"nan"`. |
| `parse_bool` | `(String) -> Result<Bool, String>` | Parse `"true"`/`"false"` (case-insensitive). Returns `Err` for any other input. |
| `float_to_str` | `(Float) -> String` | Convert float to string representation. |
| `bool_to_str` | `(Bool) -> String` | Convert bool to `"true"` or `"false"`. |
| `int_to_float` | `(Int) -> Float` | Widen integer to float (always succeeds). |
| `float_to_int` | `(Float) -> Int` | Truncate float to integer (always succeeds — truncates toward zero). |

Note: `int_to_str` and `str` already exist. `int_to_float` and `float_to_int` are new.

### 7.3 Usage with `try`/`catch`

Parsing functions integrate directly with the existing error handling model:

```sage
// Propagate error to on error handler
let n = try parse_int(response.body)
let doubled = n * 2

// Provide a default value on failure
let n = parse_int(raw) catch { 0 }

// Explicit match
match parse_int(raw) {
    Ok(n)  => print("Got: {n}")
    Err(e) => print("Bad number: {e}")
}
```

### 7.4 Codegen

```sage
parse_int(s)
```
→
```rust
s.trim().parse::<i64>().map_err(|e| e.to_string())
```

```sage
parse_float(s)
```
→
```rust
s.trim().parse::<f64>().map_err(|e| e.to_string())
```

```sage
parse_bool(s)
```
→
```rust
match s.to_lowercase().trim() {
    "true"  => Ok(true),
    "false" => Ok(false),
    other   => Err(format!("cannot parse '{}' as Bool", other)),
}
```

---

## 8. Feature 5 — Modulo Operator `%`

### 8.1 The gap

`%` (remainder) is missing from Sage's operator set. The BinOp enum has `Add`, `Sub`, `Mul`, `Div`, but no `Rem`. This means every parity check, time calculation, circular index, or hash function requires a verbose workaround.

### 8.2 Semantics

`%` is the **remainder** operator, not the mathematical modulo. Semantics follow Rust and C: the result has the same sign as the dividend.

```sage
10 % 3   // 1
-10 % 3  // -1  (NOT 2)
10 % -3  // 1
-10 % -3 // -1
```

This matches developer expectations coming from C, Java, JavaScript, Go, and Rust. The difference between remainder and modulo only matters for negative numbers. If Python-style modulo (`result always non-negative`) is needed, it can be expressed as `((n % m) + m) % m`, or added as `math_mod(n, m)` as a stdlib function in a future RFC.

`%` is supported for both `Int` and `Float`. Division by zero at runtime causes a panic (same as `/`) — no special casing is added for the first release (see §15.2).

### 8.3 Operator precedence

`%` has the same precedence as `*` and `/` — higher than `+`/`-`, lower than unary operators. This matches every other language.

Updated precedence table (additions in bold):

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 7 | `*`, `/`, **`%`** | Left |
| 6 | `+`, `-` | Left |
| 5 | `++` (concat) | Left |
| 4 | `<`, `>`, `<=`, `>=` | Left |
| 3 | `==`, `!=` | Left |
| 2 | `&&` | Left |
| 1 | `\|\|` | Left |

### 8.4 Changes required

**Lexer:** `%` is already a valid character. A `Token::Percent` token needs to be added to the logos token enum.

**Parser:** `BinOp::Rem` is added to the `BinOp` enum. The binary expression parser adds `%` at the same precedence level as `*` and `/`.

**Checker:** Type rules for `%` mirror `/`:
- `Int % Int -> Int` (E046 if right operand is a literal `0`)
- `Float % Float -> Float`
- `Int % Float` and `Float % Int` are type errors (E003) — explicit cast required.

**Codegen:** `BinOp::Rem` emits `%` directly. No transformation needed.

---

## 9. Feature 6 — String Interpolation Improvements

### 9.1 Current limitations

Sage's string interpolation currently supports only bare identifiers: `"Hello, {name}"`. Real programs constantly need to interpolate record fields (`{person.name}`), tuple fields (`{pair.0}`), and map lookups. Without this, every interpolation involving a non-local value requires a `let` extraction:

```sage
// Current: verbose extraction required
let name = person.name
let age = person.age
print("Hello, {name}! Age: {age}")

// Wanted: direct field access in interpolation
print("Hello, {person.name}! Age: {person.age}")
```

This RFC extends interpolation to support a constrained set of expression types within `{ }`. The full expression interpolation common in languages like Kotlin, Swift, and Dart is explicitly **not** in scope — only the access patterns that are needed day-to-day and that have unambiguous syntax.

### 9.2 Supported interpolation expressions

The following expression forms are supported inside `{ }`:

**Bare identifier (existing):**
```sage
"Hello, {name}"
```

**Single field access:**
```sage
"Hello, {person.name}"
"Agent: {self.topic}"
```

**Nested field access (up to 3 levels):**
```sage
"Status: {result.meta.status}"
```

**Tuple index access:**
```sage
"First: {pair.0}, Second: {pair.1}"
```

**Function call with no arguments (new — limited):**
```sage
"Length: {items.len()}"   // Only zero-argument method-style calls
```

Wait — this is out of scope. Method calls on values are not yet part of the Sage language (there is no `items.len()` method syntax). This is deferred. The supported set is:

- `{ident}` — bare identifier (existing)
- `{ident.field}` — single field access on identifier
- `{self.field}` — field access on `self` (agent context)
- `{ident.field.field}` — nested field access (up to 3 levels; deeper nesting is a parse error suggesting extraction to a `let` binding)
- `{ident.0}`, `{ident.1}`, etc. — tuple index access on identifier

Expressions with operators (`{a + b}`), function calls (`{str(n)}`), and conditional expressions (`{if x { y } else { z }}`) are **not supported** and are a parse error. This is intentional. The right tool for complex interpolation is a `let` binding.

### 9.3 Syntax

The interpolated expression inside `{ }` is parsed as a restricted access expression. The lexer recognises `{`, then the parser reads the expression with a limited grammar, then requires `}`. The grammar for interpolation expressions:

```
interp_expr ::= ident ("." (ident | INT_LIT))*
```

Where `INT_LIT` is a non-negative integer literal used for tuple indexing. The chain length limit of 3 field accesses is a checker rule (E047), not a parser rule — the parser accepts any length.

### 9.4 Changes to the AST

`StringPart::Interpolation` currently holds an `Ident`. It is extended to hold a richer expression type:

```rust
// Before (RFC-0001):
pub enum StringPart {
    Literal(String),
    Interpolation(Ident),
}

// After (RFC-0013):
pub enum StringPart {
    Literal(String),
    Interpolation(InterpExpr),
}

/// A restricted expression valid inside string interpolation.
pub enum InterpExpr {
    /// `{name}` — bare identifier
    Ident(Ident),
    /// `{person.name}`, `{self.field}`, `{result.meta.status}`
    FieldAccess {
        base: Box<InterpExpr>,
        field: Ident,
        span: Span,
    },
    /// `{pair.0}`, `{triple.2}`
    TupleIndex {
        base: Box<InterpExpr>,
        index: usize,
        span: Span,
    },
}
```

### 9.5 Codegen for extended interpolation

The codegen for `StringPart::Interpolation` already emits a `format!` argument. Extended interpolation simply emits a more complex expression:

| Interpolation | Generated Rust |
|---------------|----------------|
| `{name}` | `name` |
| `{person.name}` | `person.name.clone()` |
| `{self.topic}` | `self.topic.clone()` |
| `{result.meta.status}` | `result.meta.status.clone()` |
| `{pair.0}` | `pair.0.clone()` |

String values are cloned (consistent with the existing codegen model). Non-string values use `to_string()` or the existing `Display` implementation.

### 9.6 Type checking extended interpolation

The checker resolves the type of an `InterpExpr` the same way it resolves a regular access expression:

- `InterpExpr::Ident(name)` — look up `name` in scope, same as today.
- `InterpExpr::FieldAccess { base, field }` — resolve `base`, then look up `field` on the resulting record type (error E009 if not a record, E016 if field does not exist).
- `InterpExpr::TupleIndex { base, index }` — resolve `base`, check it is a tuple type, check `index` is in bounds (error E034 if not).

The resolved type must be displayable (same constraint as the existing `str()` builtin — any type can be converted to a string, so any type is valid in interpolation). This is already the case for all Sage types.

---

## 10. Feature 7 — `sage new` CLI Command

### 10.1 The gap

`sage new` is declared in `sage-cli`'s `Commands` enum with an empty body (`[ ]`). It has been a stub since RFC-0002. Running it does nothing. This is confusing for new users who try to start a project.

### 10.2 Behaviour

```
sage new <name>
```

Creates a directory named `<name>` in the current working directory and writes four files:

**`<name>/sage.toml`:**
```toml
[project]
name = "<name>"
version = "0.1.0"
entry = "src/main.sg"

[dependencies]
```

**`<name>/src/main.sg`:**
```sage
agent Main {
    on start {
        print("Hello from <name>!")
        emit(0)
    }
}

run Main
```

**`<name>/.gitignore`:**
```
target/
.sage/
*.lock
```

**`<name>/README.md`:**
```markdown
# <name>

A Sage project.

## Running

```
sage run src/main.sg
```
```

That is all. No options, no interactive prompts, no templates beyond this one. The scaffold is minimal and immediately runnable with `sage run src/main.sg`.

### 10.3 Error handling

- If `<name>` is missing: print usage, exit 1.
- If the directory already exists: print `"error: directory '<name>' already exists"`, exit 1.
- If a file write fails: print the OS error, exit 1.
- Name validation: only alphanumeric characters, hyphens, and underscores. Print `"error: project name '<name>' contains invalid characters"` for anything else.

### 10.4 CLI changes

The existing stub in `sage-cli/src/main.rs`:

```rust
// Existing stub — Commands enum
New { name: String },

// Handler — currently empty
Commands::New { name } => { /* TODO */ }
```

Is implemented:

```rust
Commands::New { name } => new_project(&name)?,
```

The `new_project` function is ~40 lines of `std::fs` calls. No new dependencies needed.

---

## 11. Checker Rules

| Code | Rule |
|------|------|
| E045 | `str_len` called on non-`String` type |
| E046 | Division or remainder by literal zero (e.g. `n % 0`) — warning, not error |
| E047 | String interpolation access chain too deep (> 3 levels) — use a `let` binding |
| E048 | String interpolation contains unsupported expression form (operators, function calls) |

Note: E040 was reassigned in RFC-0011's notes (from RFC-0009's closure parameter error). The original RFC-0009 E040 code should be confirmed against the master error code list and renumbered consistently.

All other type errors for new builtins reuse existing codes: E003 (type mismatch), E004 (wrong argument count), E002 (undefined name).

---

## 12. Codegen

### 12.1 String functions → Rust `str` methods

All string builtins map directly to Rust `str` or `String` methods:

| Sage | Generated Rust |
|------|----------------|
| `split(s, delim)` | `s.split(&*delim).map(str::to_string).collect::<Vec<_>>()` |
| `trim(s)` | `s.trim().to_string()` |
| `trim_start(s)` | `s.trim_start().to_string()` |
| `trim_end(s)` | `s.trim_end().to_string()` |
| `starts_with(s, p)` | `s.starts_with(&*p)` |
| `ends_with(s, p)` | `s.ends_with(&*p)` |
| `replace(s, from, to)` | `s.replace(&*from, &*to)` |
| `replace_first(s, from, to)` | `s.replacen(&*from, &*to, 1)` |
| `to_upper(s)` | `s.to_ascii_uppercase()` |
| `to_lower(s)` | `s.to_ascii_lowercase()` |
| `str_len(s)` | `s.chars().count() as i64` |
| `str_slice(s, a, b)` | `s.chars().skip(a as usize).take((b-a) as usize).collect::<String>()` |
| `str_index_of(s, needle)` | `s.char_indices().zip(s.match_indices(&*needle)).next().map(\|(i, _)\| i as i64)` (simplified — actual implementation uses `find` and char counting) |
| `str_repeat(s, n)` | `s.repeat(n.max(0) as usize)` |
| `str_pad_start(s, n, pad)` | `format!("{:>width$}", s, width = n as usize)` (pad char handling requires manual implementation for non-space pads) |
| `str_pad_end(s, n, pad)` | Symmetric to `str_pad_start` |

`str_index_of` requires a helper in `sage-runtime` because computing the code point index of a substring in a Rust `&str` requires walking the char sequence. The codegen emits a call to `sage_runtime::str_index_of(s, needle)` rather than inlining the logic.

`str_pad_start` and `str_pad_end` with custom pad strings also emit `sage_runtime` helpers rather than complex inline expressions.

### 12.2 Math functions → Rust `i64`/`f64` methods

| Sage | Generated Rust |
|------|----------------|
| `abs(n)` | `n.abs()` |
| `abs_float(n)` | `n.abs()` |
| `min(a, b)` | `a.min(b)` |
| `max(a, b)` | `a.max(b)` |
| `clamp(v, lo, hi)` | `v.clamp(lo, hi)` |
| `floor(n)` | `n.floor() as i64` |
| `ceil(n)` | `n.ceil() as i64` |
| `round(n)` | `n.round() as i64` |
| `floor_float(n)` | `n.floor()` |
| `ceil_float(n)` | `n.ceil()` |
| `pow(a, b)` | `a.pow(b as u32)` (with check: negative `b` returns `0`) |
| `pow_float(a, b)` | `a.powf(b)` |
| `sqrt(n)` | `n.sqrt()` |
| `int_to_float(n)` | `n as f64` |
| `float_to_int(n)` | `n as i64` |

### 12.3 `%` operator → Rust `%`

```sage
n % m
```
→
```rust
n % m
```

Direct passthrough. No transformation needed. Same for floats.

### 12.4 Where helpers live

Some builtins are simple enough to inline (all the math functions, most string functions). Others require helpers in `sage-runtime` to avoid generating verbose inline code:

**New helpers in `sage-runtime/src/stdlib/`:**
- `sage_runtime::stdlib::str_index_of(s: &str, needle: &str) -> Option<i64>`
- `sage_runtime::stdlib::str_pad_start(s: &str, n: i64, pad: &str) -> String`
- `sage_runtime::stdlib::str_pad_end(s: &str, n: i64, pad: &str) -> String`

Everything else is inlined by the codegen.

---

## 13. New Error Codes

| Code | Name | Description |
|------|------|-------------|
| E045 | `StrLenOnNonString` | `str_len` called on a non-`String` value |
| E046 | `RemainderByZero` | Literal zero used as right operand of `%` or `/` (warning) |
| E047 | `InterpChainTooDeep` | String interpolation access chain exceeds 3 levels |
| E048 | `InterpUnsupportedExpr` | String interpolation contains an unsupported expression (operators, calls, etc.) |

---

## 14. Implementation Plan

### Phase 1 — Operator and parsing (3–4 days)

**`%` operator:**
- Add `Token::Percent` to `sage-lexer` token enum
- Add `BinOp::Rem` to `sage-parser` AST
- Update the expression parser to handle `%` at `*`/`/` precedence
- Update the checker to type-check `Int % Int -> Int` and `Float % Float -> Float`
- Update codegen to emit `%`
- Add E046 check for literal-zero right operand
- Tests: basic arithmetic, precedence, negative operands, E046 warning

**Parsing functions:**
- Register `parse_int`, `parse_float`, `parse_bool` signatures in the checker's symbol table
- Add codegen cases for each
- Tests: happy path, error path, integration with `try`/`catch`

**`int_to_float`, `float_to_int`:**
- Register signatures
- Codegen: `n as f64`, `n as i64`
- Tests

### Phase 2 — String functions (3–4 days)

- Register all 16 string functions in checker symbol table
- Add codegen cases for each (inline or runtime helper call)
- Implement `sage_runtime::stdlib::str_index_of`, `str_pad_start`, `str_pad_end`
- Add E045 check for `str_len` on non-string
- Tests: one test per function covering normal case, edge cases (empty string, out-of-bounds, full match), and the checker error

### Phase 3 — Math functions and constants (2 days)

- Add `PI` and `E` as prelude constants (requires the checker to handle constant expressions — these are literal floats, so no evaluation needed)
- Register all 17 math functions in checker symbol table
- Add codegen cases (all inline — no runtime helpers needed)
- Tests: each function including boundary cases (`abs(Int::MIN)`, `floor(-1.5)`, `clamp` with inverted bounds)

### Phase 4 — List HOFs (4–5 days)

- Register all 18 HOF signatures in checker symbol table
- Implement generic type inference for `map`, `filter`, `reduce`, `find`, `flat_map`, `zip`, `enumerate`, `sort_by`, `unique` in the checker
- Add codegen cases (all inline iterator chains — no runtime helpers needed)
- Tests: type inference tests (checker accepts correct signatures, rejects wrong ones), runtime tests for each function including empty list behaviour, `zip` length mismatch, `sort_by` with all orderings

### Phase 5 — String interpolation improvements (3–4 days)

- Extend `StringPart::Interpolation` to hold `InterpExpr` instead of `Ident`
- Update lexer string template parsing to handle `{ident.field}`, `{ident.0}` inside string literals
- Add `InterpExpr` AST type with `Ident`, `FieldAccess`, `TupleIndex` variants
- Update the checker to type-check `InterpExpr` chains (field access on records, tuple index on tuples)
- Add E047 (chain too deep) and E048 (unsupported expression) checker errors
- Update codegen to emit extended field access and tuple index expressions inside `format!`
- Update the LSP TextMate grammar (in `vscode-sage`) to highlight `{ident.field}` interpolations correctly — the current regex `\\{[a-zA-Z_][a-zA-Z0-9_]*\\}` must be extended to `\\{[a-zA-Z_][a-zA-Z0-9_.]*[a-zA-Z0-9_]*\\}` or similar
- Tests: field access interpolation, nested field access, tuple index interpolation, error on unsupported expression, error on chain too deep

### Phase 6 — `sage new` (1 day)

- Implement `new_project(name: &str) -> Result<()>` in `sage-cli`
- Create directory, write four files
- Add name validation
- Add E messages for existing directory, invalid name
- Tests: successful scaffold, invalid name, existing directory

### Phase 7 — Tests and polish (2 days)

- End-to-end test programs using the new stdlib (a realistic program that strings, filters, maps, formats, and parses)
- Update `guide/src/language/functions.md` with the new builtin table
- Add stdlib reference page to guide: `guide/src/stdlib.md`
- Update LSP TextMate grammar for new interpolation syntax
- Ensure `sage check` errors on all new error codes with helpful messages

**Total estimated effort: 3–4 weeks**

---

## 15. Open Questions

### 15.1 Unicode handling in string functions

`to_upper` and `to_lower` use `to_ascii_uppercase()` and `to_ascii_lowercase()` — they only transform ASCII letters. Non-ASCII letters (é, ü, ñ, etc.) are passed through unchanged. This is intentional for the first release: Unicode case folding is complex, locale-dependent, and adds a dependency (`unicode-normalization` or similar). Most LLM output and agent-to-agent data is ASCII-heavy.

The tradeoff is that `to_upper("héllo")` returns `"HéLLO"` — the é is not uppercased. This is clearly documented. A future RFC can add `to_upper_unicode`/`to_lower_unicode` with proper Unicode support when there is demand.

### 15.2 Division and remainder by zero

Currently `n / 0` panics at runtime with a Rust integer overflow panic. This is the same behaviour `n % 0` will have. The ideal behaviour is a runtime `Error` propagated through the `on error` handler, not a panic. However, implementing division-by-zero as a `Result` return type would require changing the type of `/` itself, which is a breaking change.

A pragmatic middle ground: add runtime checks in the generated code that convert division-by-zero into a `SageError::Runtime` and surface it through the existing error handling mechanism. This is a non-breaking enhancement. Deferred to a follow-up RFC that addresses runtime panics generally.

### 15.3 `sort_by` with unstable sort

Rust's `sort_by` is stable (preserves order of equal elements). The generated `Vec::sort_by` is also stable. This is documented behaviour. If unstable sort is needed for performance in a future release, `sort_by_unstable` can be offered as a separate function.

### 15.4 `sum` and `sum_floats` overflow

`sum` on a list of large integers can overflow silently (Rust `i64` wrapping semantics in release builds). The same issue affects arithmetic generally in Sage. Overflow detection is a cross-cutting concern deferred to a future RFC on numeric safety.

### 15.5 Interpolation with method calls deferred

`{items.len()}` (method-call interpolation with no arguments) was considered and explicitly deferred. The reason: Sage does not yet have method call syntax on values — there is no `items.len()` expression in the language outside of string interpolation. Adding it only inside string templates would create an inconsistency. When method calls are added to the language (a future RFC), they will automatically become valid in interpolation positions via the `InterpExpr` extension mechanism in §9.4.

---

## 16. Alternatives Considered

### 16.1 Method syntax instead of functions (`s.split(",")` vs `split(s, ",")`)

Python, Kotlin, Swift, and Dart use method syntax for string operations. Most Rust developers expect method syntax too.

**Deferred, not rejected.** Method call syntax on values (UFCS or OOP-style) is a significant language feature that affects the parser, checker, and codegen broadly. It is out of scope for this RFC. When method calls are added, the builtin functions in this RFC can be made available as methods on their respective types. The free-function form (`split(s, ",")`) will continue to work regardless — it is not being retired.

### 16.2 `sort` instead of `sort_by`

A simple `sort(List<A>) -> List<A>` that works on any comparable type was considered.

**Deferred.** It requires either a `Comparable` constraint mechanism (user-defined generics with bounds — a future RFC) or special-casing each comparable type in the checker. `sort_by` with a comparator closure is more general and works immediately with the existing type system. A convenience `sort` for `List<Int>`, `List<Float>`, and `List<String>` can be added as special-cased builtins later.

### 16.3 `Option`-returning `str_slice` instead of empty-string-on-OOB

`str_slice("hello", 10, 20)` returns `""` rather than `None` or an error. An `Option<String>` return would be safer.

**Rejected.** The dominant use case for `str_slice` is extracting known substrings (e.g., the first N characters of an LLM response). Forcing every call to handle an `Option` adds noise for the common case where the programmer knows the string is long enough. If bounds safety is needed, the programmer can check `str_len(s) > start` first. The empty-string fallback matches Python's `s[10:20]` semantics, which is what most developers expect.

### 16.4 `printf`-style formatting

A `format(template, args...)` function with `%d`, `%s`, `%f` placeholders.

**Rejected.** Sage already has string interpolation via `{ }` in string literals. A separate format function would be redundant and inconsistent. The interpolation improvements in this RFC (field access, tuple index) close the practical gap. If a formatting function is ever needed, it would use Rust-style format specifiers (`{:.2}` for floats) rather than printf syntax — but this is deferred until there is a clear use case.

---

*The stdlib is where the language becomes useful. Ward splits the string. Ward filters the list. Ward finds the answer.*
