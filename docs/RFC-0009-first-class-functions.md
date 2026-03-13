# RFC-0009: First-Class Functions and Closures

- **Status:** Implemented
- **Created:** 2026-03-13
- **Author:** Pete Pavlovski
- **Depends on:** RFC-0001 (Core Language), RFC-0005 (User-Defined Types)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Design Goals](#3-design-goals)
4. [Syntax](#4-syntax)
5. [Type System](#5-type-system)
6. [Semantics](#6-semantics)
7. [Higher-Order Builtins](#7-higher-order-builtins)
8. [Checker Rules](#8-checker-rules)
9. [Codegen](#9-codegen)
10. [New Error Codes](#10-new-error-codes)
11. [Implementation Plan](#11-implementation-plan)
12. [Open Questions](#12-open-questions)
13. [Alternatives Considered](#13-alternatives-considered)
14. [Out of Scope](#14-out-of-scope)

---

## 1. Summary

This RFC introduces **first-class functions** and **closures** to Sage. Functions become values that can be stored in variables, passed as arguments, and returned from other functions. Closures capture their enclosing scope by value. A `Fn` type constructor is added to the type system. Three higher-order builtins — `map`, `filter`, and `fold` — are added to the prelude to make the feature immediately useful.

---

## 2. Motivation

Today, Sage has named function declarations but functions are not values. You cannot pass a function to another function, store it in a variable, or write a lambda inline. This creates a hard ceiling on abstraction:

```sage
// Today: you want to transform a list of topics before spawning researchers.
// There is no way to abstract this — you must write it out every time.
fn summarise_topics(topics: List<String>) -> List<String> {
    let results: List<String> = [];
    for t in topics {
        let r = t ++ " (summarised)";
        results = push(results, r);
    }
    return results;
}
```

With first-class functions and `map`:

```sage
let summarised = map(topics, |t| t ++ " (summarised)");
```

The impact is broader than convenience. Without first-class functions:

- **No `map`, `filter`, `fold`** — list processing requires explicit loops everywhere.
- **No callback patterns** — you cannot write an agent that accepts a user-supplied strategy.
- **No agent factories** — you cannot write `fn make_researcher(prompt_fn: Fn(String) -> String) -> ...`.
- **No composable prompt construction** — you cannot pass prompt-building logic around as a value.

Every real Sage program that works with lists or wants to parameterise behaviour by strategy is blocked.

---

## 3. Design Goals

1. **Closures are the primary form.** Inline `|params| body` syntax for single-expression and block closures. Named functions remain the declaration form.
2. **Named functions are values.** A function declared with `fn` can be used as a value anywhere a `Fn` type is expected.
3. **Capture by value.** Closures capture their environment by copying values at the point of creation. No references, no borrow checker complexity. Maps cleanly to Rust `move` closures.
4. **`Fn` type is explicit and readable.** `Fn(Int, String) -> Bool` — no symbolic magic, consistent with the language's keyword-heavy style.
5. **Type inference on closure parameters.** When a closure is passed to a function with a known `Fn` type, parameter type annotations may be omitted.
6. **Immediately useful.** Ship `map`, `filter`, and `fold` as builtins alongside this RFC so the feature has a compelling demo on day one.

---

## 4. Syntax

### 4.1 Closure Expressions

A closure is written with a parameter list between pipes, followed by an expression or block body:

```sage
// Single-expression closure — implicit return
|x: Int| x * 2

// Block closure — explicit return
|x: Int| {
    let doubled = x * 2;
    return doubled;
}

// Multiple parameters
|x: Int, y: Int| x + y

// No parameters
|| print("hello")

// With type annotation on the variable
let double: Fn(Int) -> Int = |x: Int| x * 2;
```

### 4.2 Omitting Parameter Types

When the expected `Fn` type is known from context (function argument, variable annotation), parameter types may be omitted:

```sage
// Type of |x| is inferred from map's second parameter
let doubled = map(numbers, |x| x * 2);

// Type known from annotation — parameter type inferred
let double: Fn(Int) -> Int = |x| x * 2;
```

If the type cannot be inferred, the checker requires explicit annotations (E040).

### 4.3 Named Functions as Values

A function declared at top level with `fn` can be used as a value by referencing its name:

```sage
fn double(x: Int) -> Int {
    return x * 2;
}

let f: Fn(Int) -> Int = double;
let results = map(numbers, double);
```

### 4.4 The `Fn` Type

Function types are written as `Fn(ParamTypes...) -> ReturnType`:

```sage
Fn(Int) -> Int                         // takes Int, returns Int
Fn(String, Int) -> Bool                // takes String and Int, returns Bool
Fn(Int) -> Fn(Int) -> Int              // takes Int, returns a function
Fn() -> String                         // no parameters, returns String
```

`Fn` is right-associative in return position, so `Fn(Int) -> Fn(Int) -> Int` parses as `Fn(Int) -> (Fn(Int) -> Int)`.

### 4.5 Calling a Function Value

Calling a closure or function value uses the same call syntax as named functions, but the callee is an expression:

```sage
let double: Fn(Int) -> Int = |x| x * 2;
let result = double(5);                 // 10

// Calling a parameter directly
fn apply(f: Fn(Int) -> Int, x: Int) -> Int {
    return f(x);
}
```

---

## 5. Type System

### 5.1 New `TypeExpr` Variant

`sage-types/src/ty.rs` gains one new variant:

```rust
pub enum TypeExpr {
    // ... existing variants ...

    /// Function type: `Fn(A, B) -> C`
    /// The Vec holds parameter types; the Box holds the return type.
    Fn(Vec<TypeExpr>, Box<TypeExpr>),
}
```

Display: `Fn(Int, String) -> Bool`

### 5.2 New `Type` Variant

`sage-checker/src/types.rs` gains the corresponding resolved variant:

```rust
pub enum Type {
    // ... existing variants ...

    /// Function type: parameter types and return type.
    Fn(Vec<Type>, Box<Type>),
}
```

`Type::Fn` participates in `is_compatible_with` — two function types are compatible iff their parameter lists and return types are pairwise compatible.

### 5.3 Type Inference for Closures

When checking a closure expression:

1. If the closure has explicit parameter type annotations, use them directly.
2. If the closure is missing some or all annotations but there is an **expected type** in context (from a variable annotation or a function's parameter type), fill in the missing types from the expected `Fn` type.
3. If types cannot be determined either way, emit E040.

The return type of a closure is always inferred from the body — it is never written explicitly by the programmer. (A future RFC may add explicit return type annotation on closures if needed, but inference is sufficient for the common case.)

---

## 6. Semantics

### 6.1 Capture by Value

Closures capture every free variable in their body by **copying** the value at the point the closure is created. Mutations to the original variable after closure creation are not visible inside the closure, and mutations inside the closure are not visible outside.

```sage
let x = 10;
let add_x = |n: Int| n + x;   // captures x = 10
x = 99;                         // does not affect add_x
let result = add_x(5);          // 15, not 104
```

This is the simplest and safest semantic for a language without references. It maps directly to Rust `move` closures, which own their captured values.

### 6.2 Closures are Not Recursive

A closure cannot refer to itself by name — there is no name to refer to. Recursive behaviour must be expressed with named `fn` declarations. If recursive closures are needed in the future, they can be addressed separately (e.g., a `fix` combinator or a `rec` keyword).

### 6.3 Named Functions as Values

When a named function is used as a value (assigned to a variable or passed as an argument), it behaves exactly as a closure that captures nothing. The function's declared type `fn add(x: Int, y: Int) -> Int` corresponds to the value type `Fn(Int, Int) -> Int`.

### 6.4 Higher-Order Functions in User Code

Without generics (deferred to a later RFC), user-defined higher-order functions are limited to concrete types:

```sage
fn apply_twice(f: Fn(Int) -> Int, x: Int) -> Int {
    return f(f(x));
}

let result = apply_twice(|n| n + 1, 5);  // 7
```

Generic higher-order functions (`fn map<T, U>(list: List<T>, f: Fn(T) -> U) -> List<U>`) require generics. In the interim, `map`, `filter`, and `fold` are handled as builtins with special type-checking logic, exactly as `push` and `len` are today.

---

## 7. Higher-Order Builtins

Three builtins are added to the prelude. All three use special type-checking logic (no generics required in the checker) analogous to how `push` and `len` already handle their generic behaviour.

### 7.1 `map`

```
map(list: List<T>, f: Fn(T) -> U) -> List<U>
```

Applies `f` to each element of `list` and returns a new list of the results.

```sage
let numbers = [1, 2, 3, 4, 5];
let doubled = map(numbers, |x| x * 2);          // [2, 4, 6, 8, 10]
let as_strings = map(numbers, |x| str(x));       // ["1", "2", "3", "4", "5"]
```

### 7.2 `filter`

```
filter(list: List<T>, pred: Fn(T) -> Bool) -> List<T>
```

Returns a new list containing only the elements of `list` for which `pred` returns `true`.

```sage
let numbers = [1, 2, 3, 4, 5, 6];
let evens = filter(numbers, |x| x % 2 == 0);   // [2, 4, 6]
```

### 7.3 `fold`

```
fold(list: List<T>, init: U, f: Fn(U, T) -> U) -> U
```

Reduces `list` to a single value by applying `f` to an accumulator (starting at `init`) and each element in turn.

```sage
let numbers = [1, 2, 3, 4, 5];
let sum = fold(numbers, 0, |acc, x| acc + x);          // 15
let product = fold(numbers, 1, |acc, x| acc * x);       // 120
let joined = fold(["a", "b", "c"], "", |acc, s| acc ++ s);  // "abc"
```

### 7.4 Checker Logic for Higher-Order Builtins

The checker handles `map`, `filter`, and `fold` with special cases, similar to the existing `push`/`len` handling:

For **`map(list, f)`**:
1. Check `list` is `List<T>` for some `T`. Extract `T`.
2. Check `f` is `Fn(T) -> U` for some `U`. The expected type for `f`'s parameter is `T` — this drives closure type inference.
3. Return type is `List<U>`.

For **`filter(list, pred)`**:
1. Check `list` is `List<T>`. Extract `T`.
2. Check `pred` is `Fn(T) -> Bool`. Use `T` to drive closure inference.
3. Return type is `List<T>`.

For **`fold(list, init, f)`**:
1. Check `list` is `List<T>`. Extract `T`.
2. Infer `U` from `init`'s type.
3. Check `f` is `Fn(U, T) -> U`.
4. Return type is `U`.

---

## 8. Checker Rules

### 8.1 Closure Expression

When checking `|params| body`:

1. Build the parameter type list, filling in any omitted types from the expected `Fn` type in context.
2. If any parameter type is still unknown, emit E040 (cannot infer closure parameter types).
3. Push a new scope with the parameters bound to their types.
4. If the body is a single expression, check it and use its type as the return type.
5. If the body is a block, check all statements. The return type is the type of the last `return` expression, or `Unit` if the block ends without one.
6. Pop the scope.
7. The closure's type is `Fn(param_types...) -> return_type`.

### 8.2 Calling a Function Value

When checking a call expression where the callee is not a known named function:

1. Check the callee expression. Its type must be `Fn(param_types...) -> return_type`.
2. If the type is not `Fn(...)`, emit E041 (calling a non-function value).
3. Check that the argument count matches. If not, emit E042 (wrong argument count for function value).
4. Check each argument against the corresponding parameter type.
5. The call expression's type is `return_type`.

### 8.3 Named Function as Value

When a named function identifier appears in expression position (not as the callee of a call):

1. Look up the function in the symbol table.
2. Its value type is `Fn(param_types...) -> return_type` constructed from the function's declaration.
3. This is type-compatible with any matching `Fn` type.

### 8.4 Storing `Fn` in Agent State

Closures may be stored in agent state fields:

```sage
agent Worker {
    transform: Fn(String) -> String

    on start {
        let result = self.transform("input");
        emit(result);
    }
}

agent Main {
    on start {
        let w = spawn Worker { transform: |s| s ++ "!" };
        let result = await w;
        print(result);
        emit(0);
    }
}
```

The `transform` field's type is `Fn(String) -> String`. The spawned closure is copied into the agent's state at spawn time.

---

## 9. Codegen

### 9.1 `TypeExpr::Fn` → Rust Type

Function types compile to `Box<dyn Fn(params) -> return>` in generated Rust code:

```
Fn(Int) -> Int           →  Box<dyn Fn(i64) -> i64>
Fn(String, Int) -> Bool  →  Box<dyn Fn(String, i64) -> bool>
Fn() -> String           →  Box<dyn Fn() -> String>
```

`Box<dyn Fn(...)>` is the standard Rust idiom for owned, heap-allocated, type-erased closures. It supports `move` closures, which is required for capture-by-value semantics.

### 9.2 Closure Expressions → Rust Closures

Closures always compile to `move` closures to implement capture-by-value:

```sage
let x = 10;
let add_x: Fn(Int) -> Int = |n| n + x;
```

Compiles to:

```rust
let x: i64 = 10;
let add_x: Box<dyn Fn(i64) -> i64> = Box::new(move |n: i64| n + x);
```

Block-body closures compile to block-body Rust closures:

```sage
let f: Fn(Int) -> Int = |n| {
    let doubled = n * 2;
    return doubled;
};
```

Compiles to:

```rust
let f: Box<dyn Fn(i64) -> i64> = Box::new(move |n: i64| {
    let doubled: i64 = n * 2;
    doubled
});
```

(`return` statements inside closures compile to the final expression position in the Rust block, or an early `return` inside the closure body.)

### 9.3 Named Functions as Values

```sage
fn double(x: Int) -> Int { return x * 2; }
let f: Fn(Int) -> Int = double;
```

Compiles to:

```rust
fn double(x: i64) -> i64 { x * 2 }
let f: Box<dyn Fn(i64) -> i64> = Box::new(move |x: i64| double(x));
```

A thin wrapper closure is generated to fit the `Box<dyn Fn>` interface.

### 9.4 Calling a Function Value

```sage
let result = f(5);
```

Compiles to:

```rust
let result = f(5);
```

`Box<dyn Fn(...)>` is callable with standard Rust call syntax — no change needed.

### 9.5 Agent State with `Fn` Fields

When an agent has a field of type `Fn(...)`, the generated Rust struct holds a `Box<dyn Fn(...)>`. Since Rust requires `Send + 'static` for values moved into async tasks, closures stored in agent state must capture only `Send + 'static` values. The codegen adds these bounds:

```rust
struct Worker {
    transform: Box<dyn Fn(String) -> String + Send + 'static>,
}
```

In practice this is satisfied automatically because all Sage values (Int, Float, Bool, String, List, records, enums) are `Send + 'static`. Closures that capture only Sage values are always safe to move into agent tasks.

### 9.6 `map`, `filter`, `fold` Builtins

These compile to direct Rust iterator calls:

```sage
map(numbers, |x| x * 2)
```
```rust
numbers.iter().map(move |&x| x * 2).collect::<Vec<_>>()
```

```sage
filter(numbers, |x| x % 2 == 0)
```
```rust
numbers.iter().copied().filter(move |&x| x % 2 == 0).collect::<Vec<_>>()
```

```sage
fold(numbers, 0, |acc, x| acc + x)
```
```rust
numbers.iter().copied().fold(0i64, move |acc, x| acc + x)
```

---

## 10. New Error Codes

| Code | Name | Description |
|------|------|-------------|
| E040 | `CannotInferClosureParams` | One or more closure parameter types cannot be inferred from context and no explicit annotation was provided. |
| E041 | `CallingNonFunction` | An expression is called as a function but its type is not `Fn(...)`. |
| E042 | `WrongArgCountForFnValue` | A function value is called with the wrong number of arguments. |
| E043 | `FnTypeMismatch` | A closure or function value is used where an incompatible `Fn` type is expected. |

---

## 11. Implementation Plan

### Phase 1 — Type System (1–2 days)
- [x] Add `TypeExpr::Fn(Vec<TypeExpr>, Box<TypeExpr>)` to `sage-types/src/ty.rs`
- [x] Implement `Display` for `TypeExpr::Fn` (`Fn(Int) -> Bool`)
- [x] Add `Type::Fn(Vec<Type>, Box<Type>)` to `sage-checker/src/types.rs`
- [x] Extend `is_compatible_with` to handle `Type::Fn` (pairwise compatibility)
- [x] Extend `resolve_type` in `sage-checker/src/scope.rs` for `TypeExpr::Fn`
- [x] Update `Display` for `Type::Fn`
- [x] Tests: `Fn(Int) -> Bool` parses and displays correctly; compatibility rules

### Phase 2 — Parser (2–3 days)
- [x] Add `TypeExpr::Fn` parsing: `Fn(T, U) -> V` as a type expression
- [x] Add `Expr::Closure` to AST: `{ params: Vec<ClosureParam>, body: ClosureBody, span }`
  - `ClosureParam`: `{ name: Ident, ty: Option<TypeExpr>, span }`
  - `ClosureBody`: `Expr(Box<Expr>)` or `Block(Block)`
- [x] Parse closure syntax `|params| expr` and `|params| { block }`
- [x] Parse `||` (no-parameter closure) — careful not to conflict with `||` (logical or)
- [x] Extend `Expr::Call` to support expression callees (currently only named `Ident`)
  - Rename to `Expr::Call { callee: CallTarget, args, span }` where `CallTarget` is `Named(Ident)` or `Expr(Box<Expr>)`
  - Alternatively: add `Expr::CallExpr { callee: Box<Expr>, args, span }` as a separate variant
- [x] Parser tests: all closure forms, `Fn` type annotations, calling a variable

### Phase 3 — Type Checker (3–4 days)
- [x] Check `Expr::Closure`: parameter type resolution, scope push/pop, return type inference
- [x] Implement expected-type-driven parameter inference for closures
- [x] Check named function used as value (`Expr::Var` resolving to a function name)
- [x] Check `Expr::CallExpr` (calling a non-named callee)
- [x] Add `map`, `filter`, `fold` to `SymbolTable::register_builtins` with special-case handling
- [x] Implement checker logic for `map`, `filter`, `fold` (extract `T`/`U`, drive closure inference)
- [x] Emit E040–E043 in appropriate locations
- [x] Tests: all four error codes; type inference on closures; `map`/`filter`/`fold` type checking

### Phase 4 — Codegen (2–3 days)
- [x] Emit `Box<dyn Fn(...) + Send + 'static>` for `Type::Fn`
- [x] Emit `Box::new(move |params| body)` for `Expr::Closure`
- [x] Emit thin wrapper closures for named-function-as-value
- [x] Emit `Box::new(move |params| { ... })` for block-body closures
- [x] Handle `Expr::CallExpr` as a direct Rust call
- [x] Emit iterator chains for `map`, `filter`, `fold`
- [x] Agent struct fields of `Fn` type get `+ Send + 'static` bounds
- [x] Codegen tests: closures round-trip through compile + execute

### Phase 5 — Tests & Examples (2 days)
- [x] End-to-end test: `map` / `filter` / `fold` on `List<Int>` and `List<String>`
- [x] End-to-end test: closure stored in variable and called later
- [x] End-to-end test: higher-order function (`apply_twice`)
- [x] End-to-end test: closure stored in agent state field
- [x] End-to-end test: named function passed as value to `map`
- [x] Add `examples/closures.sg` demonstrating the feature
- [x] Update guide documentation

**Total estimated effort:** ~2.5 weeks

---

## 12. Open Questions

**Q1: Should `||` (no-parameter closure) conflict with `||` (logical or)?**

`||` appears both as the logical-or binary operator and as an empty parameter list for a closure. The parser must disambiguate by context: `||` in expression position where a value (not an operator) is expected is a closure; `||` between two boolean expressions is logical-or. This is the same disambiguation Rust makes and chumsky can handle it with lookahead. Still, it warrants careful parser testing.

**Q2: Should closures be allowed to mutate captured variables?**

Under capture-by-value semantics, mutations inside a closure are invisible outside. This RFC allows mutation of captured copies inside the closure body (the closure owns its captured values). If the programmer wants to mutate the *outer* variable via a closure, they cannot — this is intentional. A future RFC with reference types or cell-like types could revisit this.

**Q3: Should `Fn` type be `Fn` / `FnMut` / `FnOnce` (Rust-style)?**

Rust distinguishes these three. For Sage v1, all closures are `Fn` (callable multiple times, do not consume their environment). `FnOnce` (closures that move out of their captured values) is not needed because Sage values are all `Copy` or cheaply `Clone`d. `FnMut` (closures that mutate captured state) is excluded intentionally for simplicity. This can be revisited when mutation semantics are more fully specified.

**Q4: Can closures be stored in `List`?**

`List<Fn(Int) -> Int>` should be valid — it's a homogeneous list of function values. The checker handles this naturally since `Fn(...)` is just another `Type` variant. The codegen stores `Vec<Box<dyn Fn(i64) -> i64 + Send + 'static>>`. No special handling needed.

**Q5: What about `Inferred<Fn(...)>`?**

Asking an LLM to produce a function value is not meaningful — `Inferred<Fn(Int) -> Int>` should be a type error. The checker should emit a new error (E044, deferred) if `Fn` appears inside `Inferred<...>`.

---

## 13. Alternatives Considered

### 13.1 `func` keyword for closures

```sage
let f = func(x: Int) Int { return x * 2; };
```

**Rejected.** More verbose than the pipe syntax, and anonymous functions with a `func` keyword feel inconsistent with Rust/Swift/Kotlin conventions that use `|` or `->`. The pipe syntax is compact, widely understood, and feels natural next to `for` loops.

### 13.2 Arrow syntax

```sage
let f = (x: Int) -> x * 2;
```

**Rejected.** `->` is already used as the return type separator in function declarations and as the structured output separator in `infer`. Adding it as a closure arrow introduces ambiguity and visual noise.

### 13.3 Defer closures entirely, add only function references

Allow passing named functions as values but no inline closures.

**Rejected.** This provides little practical benefit — the main use cases (passing to `map`/`filter`, inline strategy patterns) all require inline closures. Without closures, named-function-as-value is mostly academic.

### 13.4 Use `Arc<dyn Fn>` instead of `Box<dyn Fn>`

`Arc` allows shared ownership. `Box` requires unique ownership but is cheaper (no reference counting).

**Decided on `Box`.** All Sage values are owned, not shared. The capture-by-value model means each closure owns its own copy of captured data. `Box` is the right fit.

---

## 14. Out of Scope

- **Generic higher-order functions** (`fn map<T, U>(...) -> ...`) — requires generics (future RFC)
- **`FnMut` closures** — closures that mutate captured state
- **`FnOnce` closures** — closures that consume captured values
- **Recursive closures** — self-referential closures
- **Currying / partial application** — syntactic sugar over returning closures
- **Async closures** — closures that can `await` inside their body (meaningful for agent callbacks; deferred)
- **`Inferred<Fn(...)>` error** — E044, deferred

---

*First-class functions are the difference between a language you write programs in and a language you write abstractions in. Ward demands `map`.*
