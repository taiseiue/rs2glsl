use syn::{File, Item};

pub fn generate(file: &File) -> Result<String, String> {
    for item in &file.items {
        if let Item::Fn(func) = item {
            if func.sig.ident == "main_image" {
                return generate_function(func);
            }
        }
    }

    Err("main_image not found".into())
}

