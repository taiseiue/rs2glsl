# rs2glsl リファレンス

rs2glslは、シェーダーを記述するためのRustのsubsetといえるDSLです。rs2glsl-transpilerを使って、コードをGLSL(OpenGL Shading Language)にトランスパイルできます。

- [構文](./syntax.ja.md)
- [標準ライブラリ](./stdlib.ja.md)
- [アダプタ](./adapters.ja.md)

## 制限事項

現在サポートしていない機能：

- `match` のパターン制限：整数リテラルと `_` のみ（OR パターン・ガード・負の整数・非整数識別子は不可）
- `impl` ブロック・メソッド呼び出し（`.method()` 形式）
- クロージャ・ラムダ
- ジェネリクス・トレイト境界
- タプル型・列挙型（`enum`）
- 文字列リテラル
- マクロ呼び出し（`#[builtin]` などのアトリビュートのみ使用可）
- 可変長引数関数
- `self` パラメータ
- 参照（`&T`）※ `&mut T` の out パラメータのみ使用可
- ラベル付き `break`/`continue`

