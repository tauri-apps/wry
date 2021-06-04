// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Detect ECMAScript")
    .build(&event_loop)
    .unwrap();
  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_initialization_script(
    r#"
        (function () {
            window.addEventListener('DOMContentLoaded', (event) => {

                var featureSupport = [
                  {
                    version: "ECMAScript 5 (2009)",
                    features: [
                      {name: "String.trim", supported: String.prototype.trim},
                      {name: "Array.isArray", supported: Array.isArray},
                      {name: "Array.forEach", supported: Array.prototype.forEach},
                      {name: "Array.map", supported: Array.prototype.map},
                      {name: "Array.filter", supported: Array.prototype.filter},
                      {name: "Array.reduce", supported: Array.prototype.reduce},
                      {name: "JSON.parse", supported: JSON.parse},
                      {name: "Date.now", supported: Date.now}
                    ]
                  },
                  {
                    version: "ECMAScript 6 (2015)",
                    features: [
                      {name: "Array.find", supported: Array.prototype.find},
                      {name: "Math.trunc", supported: Math.trunc},
                      {name: "Number.isInteger", supported: Number.isInteger}
                    ]
                  },
                  {
                    version: "ECMAScript 2016",
                    features: [
                      {name: "Array.includes", supported: Array.prototype.includes}
                    ]
                  },
                  {
                    version: "ECMAScript 2017",
                    features: [
                      {name: "String.padStart", supported: String.prototype.padStart},
                      {name: "String.padEnd", supported: String.prototype.padEnd},
                      {name: "Object.entries", supported: Object.entries},
                      {name: "Object.values", supported: Object.values},
                    ]
                  },
                  {
                    version: "ECMAScript 2018",
                    features: [
                      {name: "Promise.finally", supported: Promise.prototype.finally}
                    ]
                  },
                  {
                    version: "ECMAScript 2019",
                    features: [
                      {name: "Array.flat", supported: Array.prototype.flat},
                      {name: "Array.flatMap", supported: Array.prototype.flatMap},
                      {name: "Object.fromEntries", supported: Object.fromEntries},
                      {name: "String.trimStart", supported: String.prototype.trimStart},
                      {name: "String.trimEnd", supported: String.prototype.trimEnd},
                      {name: "Function.toString", supported: Function.prototype.toString}
                    ]
                  },
                  {
                    version: "ECMAScript 2020",
                    features: [
                      {name: "Promise.allSettled", supported: Promise.allSettled},
                      {name: "String.matchAll", supported: String.prototype.matchAll}
                    ]
                  },
                  {
                    version: "ECMAScript 2021",
                    features: [
                      {name: "String.replaceAll", supported: String.prototype.replaceAll},
                      {name: "Promise.any", supported: Promise.any}
                    ]
                  }
                ];

                var tableElement = document.getElementById('table');
                var summaryListElement = document.getElementById('summary');

                for (var i = 0; i < featureSupport.length; i++) {
                  var versionDetails = featureSupport[i];
                  var versionSupported = true;
                  tableElement.innerHTML += `<tr> <td style="width: 200px; font-weight: bold">${versionDetails.version}</td> </tr>`
                  for (var j = 0; j < versionDetails.features.length; j++) {
                    var feature = versionDetails.features[j];
                    tableElement.innerHTML += `<tr> <td style="width: 200px">${feature.name}</td> <td>${feature.supported ? '✔' : '❌'} </td> </tr>`
                    if (!feature.supported) versionSupported = false; 
                  }
                  summaryListElement.innerHTML += `<li> ${versionDetails.version}: ${versionSupported ? '✔' : '❌'}`
                }

            });
        })();
        "#)
    .with_url(
    r#"data:text/html,
    </html>
        <body>
            <h1>ECMAScript support list:</h1>
            <ul id="summary"></ul>
            <table>
                <thead>
                    <h3>Details:<h3>
                </thead>
                <tbody id="table"></tbody>
            </table>
        </body>
    </html>
    "#,
    )?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}
