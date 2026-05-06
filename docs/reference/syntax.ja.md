# 構文

## 型システム

### スカラー型

| rs2glsl | GLSL    |
|---------|---------|
| `bool`  | `bool`  |
| `i32`   | `int`   |
| `u32`   | `uint`  |
| `f32`   | `float` |

### ベクトル型

| rs2glsl | GLSL   |
|---------|--------|
| `Vec2`  | `vec2` |
| `Vec3`  | `vec3` |
| `Vec4`  | `vec4` |

ベクトルのコンストラクタは `Vec2::new(x, y)`、`Vec3::new(x, y, z)`、`Vec4::new(x, y, z, w)` を使います。
GLSL の組み込み関数 `vec2()`/`vec3()`/`vec4()` も直接呼び出せます。

```rust
let a = Vec2::new(1.0, 2.0);
let b = vec3(0.0, 0.5, 1.0);
```

### ベクトルのフィールドアクセスとスウィズル

`.x`, `.y`, `.z`, `.w` でコンポーネントにアクセスできます。複数コンポーネントのスウィズルも使用可能です。

```rust
let v = Vec3::new(1.0, 2.0, 3.0);
let x: f32 = v.x;
let xy: Vec2 = v.xy;
```

### 配列型

`[T; N]` 構文で固定長配列を表します。N は整数リテラルです。

```rust
let arr: [f32; 3] = [1.0, 2.0, 3.0];
let grid: [[f32; 2]; 3] = [[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]];
```


---
## 変数宣言

`let` 束縛で変数を宣言します。型は右辺から推論されます。明示的な型注釈も書けます。

```rust
let uv = frag_coord / resolution;       // 型推論
let t: f32 = 0.5;                        // 明示的型注釈
```

再代入が必要な変数は `let mut` にします。

```rust
let mut col = vec4(0.0, 0.0, 0.0, 1.0);
col.z = col.z + 0.1;
```

---

## 定数

`const` でコンパイル時定数を宣言します。型注釈は必須です。定数は巻き上げられ、トランスパイル結果の前方に出力されます。

```rust
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = PI * 2.0;
const LIGHT_DIR: Vec3 = vec3(0.577, 0.577, 0.577);
```
---

## グローバル変数

`static` にアトリビュートを付けることでGLSLのグローバル変数を宣言できます。

### `#[uniform]` — uniform 変数

```rust
#[uniform]
static time: f32 = 0.0;

#[uniform]
static resolution: Vec2 = ();
```

これは次のようなGLSLに対応します: `uniform float time;` / `uniform vec2 resolution;`

### `#[out]` — out 変数 (フラグメント出力など)

`out` 変数は `mut` が必要です。`#[glsl_name(name)]` で出力時の GLSL 変数名を変更できます。

```rust
#[out]
#[glsl_name(fragColor)]
static mut frag_color: Vec4 = ();
```

出力: `out vec4 fragColor;`

### `#[builtin("glsl_name")]` — GLSL 組み込み変数

`#[builtin]` を付けた `static` は宣言を出力せず、GLSL の組み込み変数名に置き換えられます。

```rust
#[builtin("gl_FragCoord")]
static frag_coord_raw: Vec4 = ();

#[builtin("iTime")]
static i_time: f32 = 0.0;
```

---
## 関数

### 通常の関数

パラメータと戻り値型を明示します。戻り値はブロックの最後の式（`return` 文でも可）です。

```rust
fn double(x: f32) -> f32 {
    x * 2.0
}

fn main_image(frag_coord: Vec2, resolution: Vec2, time: f32) -> Vec4 {
    let uv = frag_coord / resolution;
    vec4(uv.x, uv.y, sin(time), 1.0)
}
```

戻り値がない（void）場合は戻り値型を省略します。

```rust
fn setup() {
    // ...
}
```

### out パラメータ (`&mut T`)

`&mut T` 型のパラメータは GLSL の `out` 修飾子になります。関数内では `*param = value;` またはフィールド代入で書き込みます。

```rust
fn mainImage(frag_color: &mut Vec4, frag_coord: Vec2) {
    *frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}
```

出力:
```glsl
void mainImage(out vec4 frag_color, vec2 frag_coord) {
    frag_color = vec4(1.0, 0.0, 0.0, 1.0);
}
```
---

## 組み込み関数の定義 (`#[builtin]`)

`#[builtin("glsl_name")]` アトリビュートを付けた関数は、GLSL の組み込み関数（または任意の GLSL 識別子）にマッピングされます。関数本体はトランスパイル時に無視され、シグネチャのみが使われます。

```rust
#[builtin("texture")]
fn texture_sample(sampler: Sampler2D, uv: Vec2) -> Vec4 {
    unimplemented!()
}
```

これにより、rs2glslで定義していない GLSL 関数を型安全に呼び出せます。

---
## 制御構文

### if 式

`if` は文としてだけでなく、値を返す式としても使えます。両分岐の型は一致している必要があります。

```rust
let col = if uv.x > 0.5 {
    vec3(1.0, 0.2, 0.2)
} else {
    vec3(0.2, 0.2, 1.0)
};
```

`else if` チェーンも使えます。

```rust
let edge = if uv.x < 0.1 {
    1.0
} else if uv.x > 0.9 {
    1.0
} else {
    0.0
};
```

### for ループ

範囲ベースの `for` ループのみサポートしています。`..`（半開区間）と `..=`（閉区間）どちらも使用可能です。

```rust
for i in 0..3 {
    col.z = col.z + 0.2;
}

for i in 0..=5 {
    // i は 0, 1, 2, 3, 4, 5
}
```

GLSL の C スタイル for ループに変換されます。

---
## 演算子・キャスト

### 算術演算子

`+`, `-`, `*`, `/` および複合代入 `+=`, `-=`, `*=`, `/=` が使えます。

ベクトルとスカラーの演算は GLSL と同様に行えます。

```rust
let v = Vec2::new(1.0, 2.0) * 0.5;
let uv = frag_coord / resolution;
```

### 比較・論理演算子

| 演算子 | 意味       |
|--------|------------|
| `==`, `!=` | 等値比較 |
| `<`, `>`, `<=`, `>=` | 大小比較 |
| `&&`, `\|\|` | 論理 AND / OR |
| `!` | 論理否定 |

### 単項演算子

`-x`（符号反転）と `!x`（論理否定）が使えます。`u32` に対する符号反転はエラーです。

```rust
let flipped = -uv;
let outside = !inside;
```

### 型キャスト

`expr as T` で型変換できます。

| 変換             | 例                  |
|------------------|---------------------|
| `i32` → `f32`   | `i as f32`          |
| `f32` → `i32`   | `x as i32`          |
| `i32` ↔ `u32`  | `n as u32`          |
| `u32` → `f32`   | `n as f32`          |

```rust
for i in 0..10 {
    let t = i as f32 / 10.0;
}
```

---

## 構造体

`#[structlayout(vec2|vec3|vec4)]` アトリビュートを付けた構造体を定義できます。フィールドはすべて `f32` である必要があります。

```rust
#[structlayout(vec4)]
struct Color {
    #[component(0)]
    r: f32,
    #[component(1)]
    g: f32,
    #[component(2)]
    b: f32,
    #[component(3)]
    a: f32,
}
```

`#[component(idx)]` でコンポーネントの順序を指定できます（省略した場合はフィールド定義順）。

### 構造体リテラル

```rust
let col = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
```

GLSL では構造体の宣言は出力されず、基底型（`vec4` など）に透過的に変換されます。フィールドアクセスはスウィズル（`.x`, `.y` など）に置き換えられます。

### structlayout と GLSL の対応

| `#[structlayout(...)]` | 基底型  | フィールド数 |
|------------------------|---------|------------|
| `vec2`                 | `vec2`  | 最大 2     |
| `vec3`                 | `vec3`  | 最大 3     |
| `vec4`                 | `vec4`  | 最大 4     |

---
## 型エイリアス

`type` で型に別名を付けられます。エイリアスは GLSL 出力に現れません（透過的に展開されます）。

```rust
type Color = Vec4;
type Pos = Vec2;

fn main_image(frag_coord: Pos, resolution: Pos, time: f32) -> Color {
    let uv = frag_coord / resolution;
    vec4(uv.x, uv.y, sin(time), 1.0)
}
```

---

## 配列

固定長配列を宣言・初期化できます。

```rust
let weights: [f32; 3] = [0.25, 0.5, 0.25];
let v = weights[1];             // インデックスアクセス
```

繰り返し構文 `[expr; n]` も使えます。

```rust
let zeros: [f32; 4] = [0.0; 4];
```

多次元配列も使用可能です。

```rust
let matrix: [[f32; 3]; 2] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
```

---