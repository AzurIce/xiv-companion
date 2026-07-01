#![cfg(feature = "test-support")]

use dioxus::prelude::*;
use xiv_companion_render::test_support::{
    RenderSnapshotOptions, render_model_snapshot_with_options,
};
use xiv_companion_render::{Badge, BadgeVariant, Button, ButtonVariant, Icon, IconKind};

#[derive(Clone)]
struct DemoTarget {
    title: &'static str,
    count: usize,
}

fn render_demo_target(target: DemoTarget) -> Element {
    rsx! {
        section { class: "max-w-md rounded-lg border bg-card p-4 text-card-foreground shadow-sm",
            div { class: "flex items-center gap-3",
                div { class: "flex h-10 w-10 items-center justify-center rounded-lg border bg-background text-muted-foreground",
                    Icon { kind: IconKind::PackageSearch, class: "h-5 w-5" }
                }
                div { class: "min-w-0 flex-1",
                    h1 { class: "text-base font-semibold", "{target.title}" }
                    div { class: "text-sm text-muted-foreground", "render helper smoke target" }
                }
                Badge { variant: BadgeVariant::Outline, "{target.count}" }
            }
            div { class: "mt-4 flex gap-2",
                Button { variant: ButtonVariant::Primary, "Primary" }
                Button { "Secondary" }
            }
        }
    }
}

#[test]
#[ignore = "writes target/render-snapshots/demo-card.png with local Chrome"]
fn render_demo_card_snapshot() {
    let snapshot = render_model_snapshot_with_options(
        RenderSnapshotOptions::new("demo-card").with_viewport(720, 360),
        DemoTarget {
            title: "Render Snapshot",
            count: 7,
        },
        render_demo_target,
    )
    .expect("render snapshot");

    eprintln!("html: {}", snapshot.html_path.display());
    eprintln!("png: {}", snapshot.png_path.display());
}
