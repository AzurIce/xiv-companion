use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use serde_json::json;
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{ErrorEvent, Event, MessageEvent, WebSocket};
use xiv_companion::collection_bridge_protocol::{ParsedBridgeMessage, parse_bridge_message};
use xiv_companion::{CollectionItem, CollectionKind};

const BRIDGE_URL_KEY: &str = "xiv-companion-local-bridge-url";
const BRIDGE_VERIFIED_URL_KEY: &str = "xiv-companion-local-bridge-verified-url";
const MAX_ITEM_IDS_PER_REQUEST: usize = 256;

#[derive(Clone, Debug)]
pub enum BridgeUpdate {
    Connected,
    #[allow(dead_code)]
    SnapshotReady(Vec<u32>),
    Disconnected,
    Error(String),
}

pub struct CollectionBridgeConnection {
    socket: WebSocket,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(ErrorEvent)>,
    _on_close: Closure<dyn FnMut(Event)>,
}

impl CollectionBridgeConnection {
    pub fn connect(
        url: &str,
        catalog: &[CollectionItem],
        on_update: impl FnMut(BridgeUpdate) + 'static,
    ) -> Result<Rc<Self>, String> {
        let url = url.trim();
        if !url.starts_with("ws://127.0.0.1:") && !url.starts_with("ws://localhost:") {
            return Err("桥接地址必须使用本机 ws://127.0.0.1 或 ws://localhost".to_string());
        }

        let socket = WebSocket::new(url).map_err(js_error)?;
        let callback: Rc<RefCell<dyn FnMut(BridgeUpdate)>> = Rc::new(RefCell::new(on_update));
        let unlock_item_ids = catalog
            .iter()
            .filter(|item| item.kind != CollectionKind::Equipment)
            .map(|item| item.id)
            .collect::<Vec<_>>();
        let unlock_request_ids = (0..unlock_item_ids.len().div_ceil(MAX_ITEM_IDS_PER_REQUEST))
            .map(|index| format!("collection-unlocks-{index}"))
            .collect::<Vec<_>>();
        let pending_response_ids = Rc::new(RefCell::new(
            unlock_request_ids
                .iter()
                .cloned()
                .chain(["collection-sources".to_string()])
                .collect::<HashSet<_>>(),
        ));
        let collected_item_ids = Rc::new(RefCell::new(HashSet::<u32>::new()));
        let completed = Rc::new(Cell::new(false));
        let equipment_item_ids = Rc::new(
            catalog
                .iter()
                .filter(|item| item.kind == CollectionKind::Equipment)
                .map(|item| item.id)
                .collect::<HashSet<_>>(),
        );

        let open_socket = socket.clone();
        let open_callback = callback.clone();
        let open_request_ids = unlock_request_ids.clone();
        let on_open = Closure::wrap(Box::new(move |_event: Event| {
            open_callback.borrow_mut()(BridgeUpdate::Connected);
            for (id, chunk) in open_request_ids
                .iter()
                .zip(unlock_item_ids.chunks(MAX_ITEM_IDS_PER_REQUEST))
            {
                let request = json!({
                    "id": id,
                    "method": "collection.item-status",
                    "params": { "itemIds": chunk },
                });
                if let Err(error) = open_socket.send_with_str(&request.to_string()) {
                    open_callback.borrow_mut()(BridgeUpdate::Error(js_error(error)));
                    return;
                }
            }
            let sources = json!({
                "id": "collection-sources",
                "method": "collection.sources",
                "params": {},
            });
            if let Err(error) = open_socket.send_with_str(&sources.to_string()) {
                open_callback.borrow_mut()(BridgeUpdate::Error(js_error(error)));
            }
        }) as Box<dyn FnMut(Event)>);
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        let message_callback = callback.clone();
        let message_equipment_ids = equipment_item_ids.clone();
        let message_pending_ids = pending_response_ids.clone();
        let message_collected_ids = collected_item_ids.clone();
        let message_completed = completed.clone();
        let message_socket = socket.clone();
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            let Some(parsed) = parse_bridge_message(&text, &message_equipment_ids) else {
                return;
            };
            match parsed {
                ParsedBridgeMessage::EventObtained(_) => {}
                ParsedBridgeMessage::Response { id, item_ids } => {
                    if !message_pending_ids.borrow_mut().remove(&id) {
                        return;
                    }
                    message_collected_ids.borrow_mut().extend(item_ids);
                    if message_pending_ids.borrow().is_empty() {
                        message_completed.set(true);
                        let mut item_ids = message_collected_ids
                            .borrow()
                            .iter()
                            .copied()
                            .collect::<Vec<_>>();
                        item_ids.sort_unstable();
                        message_callback.borrow_mut()(BridgeUpdate::SnapshotReady(item_ids));
                        let _ = message_socket.close();
                    }
                }
                ParsedBridgeMessage::Error { id, message } => {
                    let Some(id) = id else {
                        return;
                    };
                    if !message_pending_ids.borrow_mut().remove(&id) {
                        return;
                    }
                    message_completed.set(true);
                    message_callback.borrow_mut()(BridgeUpdate::Error(message));
                    let _ = message_socket.close();
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let error_callback = callback.clone();
        let error_completed = completed.clone();
        let on_error = Closure::wrap(Box::new(move |event: ErrorEvent| {
            if error_completed.get() {
                return;
            }
            error_completed.set(true);
            let message = if event.message().is_empty() {
                "无法连接 API Bridge".to_string()
            } else {
                event.message()
            };
            error_callback.borrow_mut()(BridgeUpdate::Error(message));
        }) as Box<dyn FnMut(ErrorEvent)>);
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        let close_callback = callback;
        let close_completed = completed;
        let on_close = Closure::wrap(Box::new(move |_event: Event| {
            if !close_completed.get() {
                close_callback.borrow_mut()(BridgeUpdate::Disconnected);
            }
        }) as Box<dyn FnMut(Event)>);
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        Ok(Rc::new(Self {
            socket,
            _on_open: on_open,
            _on_message: on_message,
            _on_error: on_error,
            _on_close: on_close,
        }))
    }
}

impl Drop for CollectionBridgeConnection {
    fn drop(&mut self) {
        self.socket.set_onopen(None);
        self.socket.set_onmessage(None);
        self.socket.set_onerror(None);
        self.socket.set_onclose(None);
        let _ = self.socket.close();
    }
}

pub fn load_bridge_url() -> String {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(BRIDGE_URL_KEY).ok().flatten())
        .unwrap_or_default()
}

pub fn save_bridge_url(url: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let url = url.trim();
        let previous = storage.get_item(BRIDGE_URL_KEY).ok().flatten();
        let _ = storage.set_item(BRIDGE_URL_KEY, url);
        if previous.as_deref() != Some(url) {
            let _ = storage.remove_item(BRIDGE_VERIFIED_URL_KEY);
        }
    }
}

pub fn mark_bridge_verified(url: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(BRIDGE_VERIFIED_URL_KEY, url.trim());
    }
}

pub fn load_verified_bridge_url() -> Option<String> {
    let storage = web_sys::window()?.local_storage().ok().flatten()?;
    let configured = storage.get_item(BRIDGE_URL_KEY).ok().flatten()?;
    let verified = storage.get_item(BRIDGE_VERIFIED_URL_KEY).ok().flatten()?;
    (configured == verified && !configured.is_empty()).then_some(configured)
}

fn js_error(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "浏览器 WebSocket 操作失败".to_string())
}
