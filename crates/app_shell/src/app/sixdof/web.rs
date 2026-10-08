//! A 6-DoF mouse on a browser page, through WebHID: the page lists the
//! devices granted before, offers its chooser on request, and hands over
//! each input report, which `sixdof::hid::Decoder` turns into the events
//! the daemon would give. One device is read, the first one found, as the
//! crate does elsewhere. A browser without WebHID has no device.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use sixdof::hid::{self, Decoder};
use wasm_bindgen::prelude::*;

use super::{Shared, lock, lost, take_input};
use crate::log_panel as app_log;

#[wasm_bindgen(inline_js = r#"
let device = null;
let handlers = null;

// The logical range declared for each axis (x, y, z, then the turns about
// them), read from the collections the browser parsed: pairs of numbers,
// and a mark for each axis that has one.
function axisRanges(dev) {
  const ranges = new Int32Array(12);
  const declared = new Uint8Array(6);
  const walk = (collections) => {
    for (const c of collections ?? []) {
      for (const report of c.inputReports ?? []) {
        for (const item of report.items ?? []) {
          let usages = item.usages ?? [];
          if (item.isRange) {
            usages = [];
            for (let u = item.usageMinimum; u <= item.usageMaximum; u++) usages.push(u);
          }
          for (const u of usages) {
            const axis = (u >>> 16) === 1 ? (u & 0xffff) - 0x30 : -1;
            if (axis >= 0 && axis < 6 && item.logicalMaximum > item.logicalMinimum) {
              ranges[2 * axis] = item.logicalMinimum;
              ranges[2 * axis + 1] = item.logicalMaximum;
              declared[axis] = 1;
            }
          }
        }
      }
      walk(c.children);
    }
  };
  walk(dev.collections);
  return [ranges, declared];
}

async function take(dev) {
  if (device) return;
  const top = dev.collections.find((c) => c.usagePage === 1 && c.usage === 8)
    ?? dev.collections[0] ?? { usagePage: 0, usage: 0 };
  if (!handlers.accept(dev.vendorId, dev.productId, top.usagePage, top.usage)) return;
  device = dev;
  try {
    if (!dev.opened) await dev.open();
  } catch (error) {
    device = null;
    console.warn("the 6-DoF mouse did not open:", error);
    return;
  }
  const [ranges, declared] = axisRanges(dev);
  handlers.opened(dev.vendorId, dev.productId, dev.productName ?? "", ranges, declared);
  dev.addEventListener("inputreport", (event) => {
    if (device !== dev) return;
    const data = event.data;
    handlers.report(event.reportId, new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
  });
}

export function hid_start(accept, opened, report, gone) {
  if (!globalThis.navigator?.hid) return false;
  handlers = { accept, opened, report, gone };
  navigator.hid.addEventListener("connect", (event) => take(event.device));
  navigator.hid.addEventListener("disconnect", (event) => {
    if (event.device === device) {
      device = null;
      gone();
    }
  });
  navigator.hid.getDevices().then((devices) => devices.forEach(take));
  return true;
}

export function hid_choose(filters) {
  if (!globalThis.navigator?.hid) return false;
  navigator.hid
    .requestDevice({ filters: JSON.parse(filters) })
    .then((devices) => devices.forEach(take))
    .catch((error) => console.warn("no 6-DoF mouse chosen:", error));
  return true;
}
"#)]
extern "C" {
    fn hid_start(accept: &JsValue, opened: &JsValue, report: &JsValue, gone: &JsValue) -> bool;
    fn hid_choose(filters: &str) -> bool;
}

/// Starts listening for the device: one granted before opens at once, one
/// plugged in later when it comes.
pub(super) fn start(shared: Arc<Mutex<Shared>>, wake: impl Fn() + Send + 'static) {
    let wake: Rc<dyn Fn()> = Rc::new(wake);
    let decoder: Rc<RefCell<Option<Decoder>>> = Rc::default();
    let origin = web_time::Instant::now();

    let accept = Closure::<dyn Fn(u16, u16, u16, u16) -> bool>::new(hid::is_puck);
    let opened = {
        let (shared, decoder) = (shared.clone(), decoder.clone());
        Closure::<dyn Fn(u16, u16, String, Vec<i32>, Vec<u8>)>::new(
            move |vendor: u16, product: u16, name: String, ranges: Vec<i32>, declared: Vec<u8>| {
                let ranges: [Option<(i32, i32)>; 6] = std::array::from_fn(|axis| {
                    (declared.get(axis) == Some(&1))
                        .then(|| (ranges[2 * axis], ranges[2 * axis + 1]))
                });
                *decoder.borrow_mut() = Some(Decoder::with_ranges(vendor, product, ranges));
                let name = if name.is_empty() {
                    "USB device".to_string()
                } else {
                    name
                };
                app_log::info(format!("6-DoF mouse connected: {name}"));
                let mut state = lock(&shared);
                state.button_count = hid::button_count(vendor, product);
                state.device = Some(name);
            },
        )
    };
    let report = {
        let (shared, decoder, wake) = (shared.clone(), decoder.clone(), wake.clone());
        Closure::<dyn Fn(u8, Vec<u8>)>::new(move |id: u8, data: Vec<u8>| {
            let mut decoder = decoder.borrow_mut();
            let Some(decoder) = decoder.as_mut() else {
                return;
            };
            let mut report = Vec::with_capacity(data.len() + 1);
            report.push(id);
            report.extend_from_slice(&data);
            let mut events = std::collections::VecDeque::new();
            decoder.feed(&report, origin.elapsed(), &mut events);
            for event in events {
                take_input(event, &shared, &*wake);
            }
        })
    };
    let gone = Closure::<dyn Fn()>::new(move || {
        decoder.borrow_mut().take();
        lost(&shared, &*wake);
    });
    if !hid_start(
        &accept.into_js_value(),
        &opened.into_js_value(),
        &report.into_js_value(),
        &gone.into_js_value(),
    ) {
        tracing::debug!(target: "printcad.input", "this browser has no WebHID: no 6-DoF mouse");
    }
}

/// Opens the browser's chooser, listing multi-axis controllers and the
/// makers' devices.
pub(super) fn choose() {
    let filters = serde_json::json!([
        {"usagePage": hid::USAGE_PAGE, "usage": hid::USAGE},
    ])
    .to_string();
    if !hid_choose(&filters) {
        app_log::warn("this browser cannot reach a 6-DoF mouse: it has no WebHID");
    }
}
