# 標準ライブラリ

## 三角関数

| 関数            | GLSL 対応    |
|-----------------|--------------|
| `sin(x)`        | `sin(x)`     |
| `cos(x)`        | `cos(x)`     |
| `tan(x)`        | `tan(x)`     |
| `asin(x)`       | `asin(x)`    |
| `acos(x)`       | `acos(x)`    |
| `atan(x)`       | `atan(x)`    |
| `radians(deg)`  | `radians(x)` |
| `degrees(rad)`  | `degrees(x)` |

## 指数・対数

| 関数                | GLSL 対応       |
|---------------------|-----------------|
| `sqrt(x)`           | `sqrt(x)`       |
| `inversesqrt(x)`    | `inversesqrt(x)`|
| `exp(x)`            | `exp(x)`        |
| `exp2(x)`           | `exp2(x)`       |
| `log(x)`            | `log(x)`        |
| `log2(x)`           | `log2(x)`       |
| `pow(x, y)`         | `pow(x, y)`     |

## スカラー関数

| 関数                        | GLSL 対応            |
|-----------------------------|----------------------|
| `abs(x)`                    | `abs(x)`             |
| `sign(x)`                   | `sign(x)`            |
| `floor(x)`                  | `floor(x)`           |
| `ceil(x)`                   | `ceil(x)`            |
| `round(x)`                  | `round(x)`           |
| `fract(x)`                  | `fract(x)`           |
| `min(x, y)`                 | `min(x, y)`          |
| `max(x, y)`                 | `max(x, y)`          |
| `clamp(x, lo, hi)`          | `clamp(x, lo, hi)`   |
| `mix(x, y, a)`              | `mix(x, y, a)`       |
| `mod_(x, y)`                | `mod(x, y)`          |
| `smoothstep(edge0, edge1, x)` | `smoothstep(...)`  |

> **注意:** `mod_` は Rust の `%`（被除数の符号に従う）とは異なり、GLSL の `mod`（除数の符号に従う）と同じ挙動です。

## ベクトル関数

GLSL はオーバーロードをサポートしていますが Rust はしないため、引数の型が異なる関数はサフィックスで区別します。

| rs2glsl 関数名           | GLSL 対応          | 引数型       |
|--------------------------|--------------------|--------------|
| `length(v)`              | `length(v)`        | `Vec2`       |
| `length3(v)`             | `length(v)`        | `Vec3`       |
| `dot(a, b)`              | `dot(a, b)`        | `Vec3`       |
| `dot2(a, b)`             | `dot(a, b)`        | `Vec2`       |
| `cross(a, b)`            | `cross(a, b)`      | `Vec3`       |
| `normalize(v)`           | `normalize(v)`     | `Vec3`       |
| `normalize2(v)`          | `normalize(v)`     | `Vec2`       |
| `normalize4(v)`          | `normalize(v)`     | `Vec4`       |
| `distance(a, b)`         | `distance(a, b)`   | `Vec2`       |
| `distance3(a, b)`        | `distance(a, b)`   | `Vec3`       |
| `reflect(i, n)`          | `reflect(i, n)`    | `Vec3`       |

### ベクトルコンストラクタ

```rust
Vec2::new(x: f32, y: f32) -> Vec2
Vec3::new(x: f32, y: f32, z: f32) -> Vec3
Vec4::new(x: f32, y: f32, z: f32, w: f32) -> Vec4
```
