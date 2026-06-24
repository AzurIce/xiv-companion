use dioxus::prelude::*;

use crate::app::utils::cx;

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Outline,
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ButtonSize {
    Sm,
    Md,
    Icon,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BadgeVariant {
    Default,
    Secondary,
    Outline,
    Success,
    Warning,
}

#[component]
pub fn Button(
    #[props(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[props(default = ButtonSize::Md)] size: ButtonSize,
    #[props(default = String::new())] class: String,
    #[props(default = false)] disabled: bool,
    #[props(default = None)] r#type: Option<String>,
    #[props(default = None)] title: Option<String>,
    #[props(default = EventHandler::new(|_| {}))] onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    let variant_class = match variant {
        ButtonVariant::Primary => "bg-primary text-primary-foreground hover:bg-primary/90",
        ButtonVariant::Secondary => "bg-secondary text-secondary-foreground hover:bg-secondary/80",
        ButtonVariant::Outline => {
            "border border-input bg-background hover:bg-accent hover:text-accent-foreground"
        }
        ButtonVariant::Ghost => "hover:bg-accent hover:text-accent-foreground",
    };
    let size_class = match size {
        ButtonSize::Sm => "h-8 px-3",
        ButtonSize::Md => "h-9 px-3",
        ButtonSize::Icon => "h-8 w-8",
    };

    rsx! {
        button {
            r#type: r#type.unwrap_or_else(|| "button".to_string()),
            class: cx([
                "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm font-medium transition-colors",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2",
                "disabled:pointer-events-none disabled:opacity-50",
                variant_class,
                size_class,
                &class,
            ]),
            disabled,
            title,
            onclick: move |event| onclick.call(event),
            {children}
        }
    }
}

#[component]
pub fn Badge(
    #[props(default = BadgeVariant::Secondary)] variant: BadgeVariant,
    #[props(default = String::new())] class: String,
    children: Element,
) -> Element {
    let variant_class = match variant {
        BadgeVariant::Default => "bg-primary text-primary-foreground",
        BadgeVariant::Secondary => "bg-secondary text-secondary-foreground",
        BadgeVariant::Outline => "border border-border bg-background",
        BadgeVariant::Success => "border border-emerald-200 bg-emerald-50 text-emerald-700",
        BadgeVariant::Warning => "border border-amber-200 bg-amber-50 text-amber-700",
    };

    rsx! {
        span {
            class: cx([
                "inline-flex h-5 items-center rounded px-1.5 text-xs font-medium",
                variant_class,
                &class,
            ]),
            {children}
        }
    }
}

#[component]
pub fn Card(#[props(default = String::new())] class: String, children: Element) -> Element {
    rsx! {
        div {
            class: cx(["rounded-lg border bg-card text-card-foreground shadow-sm", &class]),
            {children}
        }
    }
}

#[component]
pub fn CardHeader(#[props(default = String::new())] class: String, children: Element) -> Element {
    rsx! {
        div { class: cx(["space-y-1 p-4 pb-2", &class]), {children} }
    }
}

#[component]
pub fn CardTitle(#[props(default = String::new())] class: String, children: Element) -> Element {
    rsx! {
        h2 { class: cx(["text-base font-semibold", &class]), {children} }
    }
}

#[component]
pub fn CardContent(#[props(default = String::new())] class: String, children: Element) -> Element {
    rsx! {
        div { class: cx(["p-4 pt-2", &class]), {children} }
    }
}

#[component]
pub fn EmptyState(
    #[props(default = None)] icon: Option<Element>,
    title: String,
    #[props(default = None)] description: Option<String>,
    #[props(default = None)] action: Option<Element>,
) -> Element {
    rsx! {
        div { class: "flex min-h-40 flex-col items-center justify-center gap-2 rounded-lg border border-dashed bg-background p-6 text-center",
            if let Some(icon) = icon {
                div { class: "text-muted-foreground", {icon} }
            }
            div { class: "text-sm font-medium", "{title}" }
            if let Some(description) = description {
                div { class: "max-w-sm text-sm text-muted-foreground", "{description}" }
            }
            if let Some(action) = action {
                div { class: "pt-2", {action} }
            }
        }
    }
}

pub fn input_class(class: &str) -> String {
    cx([
        "flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm",
        "placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        "disabled:cursor-not-allowed disabled:opacity-50",
        class,
    ])
}
