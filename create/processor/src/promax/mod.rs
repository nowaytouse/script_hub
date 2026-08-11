pub mod rule;
pub mod safety;
pub mod source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductKind {
    Surge,
    Loon,
    Shadowrocket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Product {
    pub kind: ProductKind,
    pub path: &'static str,
}

const PRODUCTS: &[Product] = &[
    Product {
        kind: ProductKind::Surge,
        path: "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
    },
    Product {
        kind: ProductKind::Surge,
        path: "modules/surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
    },
    Product {
        kind: ProductKind::Loon,
        path: "modules/loon/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).plugin",
    },
    Product {
        kind: ProductKind::Loon,
        path: "modules/loon/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).plugin",
    },
    Product {
        kind: ProductKind::Shadowrocket,
        path: "modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
    },
    Product {
        kind: ProductKind::Shadowrocket,
        path: "modules/shadowrocket/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
    },
];

pub fn published_products() -> &'static [Product] {
    PRODUCTS
}

#[cfg(test)]
mod tests {
    use super::{published_products, ProductKind};

    #[test]
    fn product_set_has_no_lite() {
        let products = published_products();
        assert!(products
            .iter()
            .all(|product| !product.path.to_ascii_lowercase().contains("lite")));
        assert_eq!(
            products
                .iter()
                .filter(|product| product.kind == ProductKind::Surge)
                .count(),
            2
        );
    }
}
