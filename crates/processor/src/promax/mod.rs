pub mod artifact;
mod compiler;
pub(crate) mod feed;
pub mod functional;
pub mod rule;
pub mod safety;
pub mod script_resources;
pub mod source;
pub mod validation;

pub use compiler::{migrate_published_rule_layout, refresh_published_index, run_adblock_manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductKind {
    Surge,
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
    use super::{ProductKind, published_products};

    #[test]
    fn product_set_contains_only_four_surge_and_shadowrocket_variants() {
        let products = published_products();
        assert_eq!(products.len(), 4);
        assert!(products.iter().all(|product| {
            !product.path.to_ascii_lowercase().contains("lite")
                && !product.path.to_ascii_lowercase().contains("loon")
                && matches!(product.kind, ProductKind::Surge | ProductKind::Shadowrocket)
        }));
        for kind in [ProductKind::Surge, ProductKind::Shadowrocket] {
            assert_eq!(
                products
                    .iter()
                    .filter(|product| product.kind == kind)
                    .count(),
                2
            );
        }
    }
}
