// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

#[macro_use]
extern crate chan;
extern crate chan_signal;
extern crate core;

use std::collections::HashSet;
use std::env;

use chan_signal::Signal;
use serde_env::from_env;

mod docker;
mod report;
mod util;

#[tokio::main]
async fn main() {
    let log_level = match env::var("LOG_LEVEL") {
        Ok(level) => {
            if level == "trace" {
                log::LevelFilter::Trace
            } else if level == "debug" {
                log::LevelFilter::Debug
            } else if level == "info" {
                log::LevelFilter::Info
            } else if level == "warn" {
                log::LevelFilter::Warn
            } else if level == "error" {
                log::LevelFilter::Error
            } else {
                log::LevelFilter::Info
            }
        }
        Err(..) => {
            log::LevelFilter::Info
        }
    };
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stdout())
        .apply().unwrap();
    log::trace!("trace log enabled");
    log::debug!("debug log enabled");
    log::info!("info log enabled");
    log::warn!("warn log enabled");
    log::error!("error log enabled");
    log::info!("node host name is {}", *util::NODE_HOST_NAME);
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    let reporter;
    match env::var("REPORTER_TYPE") {
        Ok(reporter_type) => {
            if reporter_type == "influx" {
                log::info!("report type is influx");
                let influx_config: report::InfluxConfig = from_env().expect("deserialize from env");
                let influx_url = format!("http://{}:{}", influx_config.reporter_influx_host, influx_config.reporter_influx_port);
                let client = influxdb::Client::new(influx_url, "telegraf");
                reporter = report::Reporter {
                    reporter_type: report::ReporterType::Influx,
                    influxdb_client: Some(client),
                }
            } else {
                reporter = report::Reporter {
                    reporter_type: report::ReporterType::Log,
                    influxdb_client: None,
                }
            }
        }
        Err(_) => {
            reporter = report::Reporter {
                reporter_type: report::ReporterType::Log,
                influxdb_client: None,
            };
            log::info!("reporter type is not set, default to log");
        }
    }
    // check enable modules from env
    let split: Vec<String> = std::env::var("ENABLE_MODULES").unwrap_or("".to_string()).split(",").map(|s| s.to_string()).collect();
    let modules = vec_to_set(split);
    if modules.contains(&"docker".to_string()) {
        tokio::task::spawn(async move {
            docker::docker_metrics(&reporter).await
        });
    }
    chan_select! {
        signal.recv() -> signal => {
            log::warn!("received signal: {:?}", signal)
        },
    }
}

fn vec_to_set(vec: Vec<String>) -> HashSet<String> {
    HashSet::from_iter(vec)
}
