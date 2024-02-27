use std::collections::VecDeque;
use std::error::Error;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::{Manager as _, Peripheral as _};
use btleplug::{
    api::{Central, ScanFilter},
    platform::{Adapter, Manager, Peripheral},
};

use eframe::egui;
use egui::{Color32, Context};
use egui_plot::{Legend, Line, LineStyle::Solid, Plot, PlotPoints};

use futures::stream::StreamExt;
use log::{debug, info};

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let rt = tokio::runtime::Runtime::new()?;

    let _enter = rt.enter();

    let (tx, rx) = std::sync::mpsc::channel();

    let sensor = TemperatureSendor::new(tx);
    let ui = UI::new(rx);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([600.0, 400.0]),
        default_theme: eframe::Theme::Light,
        ..Default::default()
    };

    eframe::run_native(
        "BLE Temperature",
        options,
        Box::new(|cc| {
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                rt.block_on(async {
                    sensor.run(&ctx).await.unwrap();
                });
            });

            Box::new(ui)
        }),
    )?;

    Ok(())
}

struct TemperatureSendor {
    tx: Sender<f32>,
}

impl TemperatureSendor {
    fn new(tx: Sender<f32>) -> Self {
        Self { tx }
    }

    async fn run(&self, egui_ctx: &Context) -> Result<(), Box<dyn Error>> {
        let manager = Manager::new().await?;

        // get the first bluetooth adapter
        let adapters = manager.adapters().await?;
        let central = adapters
            .into_iter()
            .nth(0)
            .ok_or(btleplug::Error::DeviceNotFound)?;

        // start scanning for devices
        central.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(Duration::from_secs(2)).await;

        // find the sensor
        let sensor = self.find_sensor(&central).await?;

        info!("connecting to sensor: {}", sensor.address());
        sensor.connect().await?;

        info!("discovering services");
        sensor.discover_services().await?;

        info!("findind temperature characteristic");
        let chars = sensor.characteristics();
        let notify_char = chars
            .iter()
            .find(|c| c.uuid == uuid_from_u16(0x2a1c))
            .ok_or(btleplug::Error::NoSuchCharacteristic)?;

        info!("subscribing to characteristic");
        sensor.subscribe(notify_char).await?;

        let mut stream = sensor.notifications().await?;

        while let Some(data) = stream.next().await {
            if let Some(temp) = self.decode(&data.value) {
                self.tx.send(temp)?;
                egui_ctx.request_repaint()
            }
        }

        Ok(())
    }

    async fn find_sensor(&self, central: &Adapter) -> Result<Peripheral, btleplug::Error> {
        for p in central.peripherals().await? {
            if p.properties()
                .await
                .unwrap()
                .unwrap()
                .local_name
                .iter()
                .any(|name| {
                    info!("discover sensor: {}", name);
                    name.contains("Temperature")
                })
            {
                return Ok(p);
            }
        }

        Err(btleplug::Error::DeviceNotFound)
    }

    fn decode(&self, buf: &[u8]) -> Option<f32> {
        if buf.len() != 5 {
            return None;
        }
        let is_fahrenheit = buf[0] == 1;

        let value = u32::from_le_bytes(buf[1..].try_into().unwrap()) & 0x00ffffff;
        debug!("temp: {}", value);

        let mut value = value as f32 / 1000.0;
        if is_fahrenheit {
            value = (value - 32.0) / 1.8;
        }

        Some(value)
    }
}

struct UI {
    rx: Receiver<f32>,
    measures: VecDeque<f32>,
}

impl UI {
    fn new(rx: Receiver<f32>) -> Self {
        Self {
            measures: VecDeque::with_capacity(10),
            rx,
        }
    }
}

impl eframe::App for UI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // receive temperature
        if let Ok(temp) = self.rx.try_recv() {
            if self.measures.len() >= self.measures.capacity() {
                self.measures.pop_front();
            }
            self.measures.push_back(temp);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("BLE Tempereture");

            let plot = Plot::new("tempereture")
                .legend(Legend::default())
                .include_y(30.0)
                .include_y(15.0)
                .show_axes([false, true])
                .show_x(false)
                .show_grid(true);

            plot.show(ui, |plot_ui| {
                let points: PlotPoints = self
                    .measures
                    .iter()
                    .enumerate()
                    .map(|(i, x)| [i as f64, *x as f64])
                    .collect();

                let line = Line::new(points)
                    .color(Color32::from_rgb(100, 200, 100))
                    .style(Solid)
                    .highlight(true)
                    .name("Tempereture");

                plot_ui.line(line);
            })
        });
    }
}
