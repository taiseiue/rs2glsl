use proc_macro::TokenStream;

/// GLSLビルトイン関数・変数へのマッピングを宣言する。トランスパイラが解釈し、
/// Rustコンパイラには no-op として渡す。
#[proc_macro_attribute]
pub fn builtin(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// GLSL uniform 変数として宣言する。
#[proc_macro_attribute]
pub fn uniform(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// GLSL out 変数として宣言する。
#[proc_macro_attribute]
pub fn out(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// struct の GLSL レイアウト（vec2/vec3/vec4）を指定する。
#[proc_macro_attribute]
pub fn structlayout(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// struct フィールドの GLSL コンポーネントインデックスを指定する。
#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// GLSL 出力変数名を指定する（アダプタ用）。
#[proc_macro_attribute]
pub fn glsl_name(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
