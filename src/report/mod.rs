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


use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct InfluxConfig {
    #[serde(default = "default_localhost")]
    pub reporter_influx_host: String,
    #[serde(default = "default_8086")]
    pub reporter_influx_port: i32,
}

fn default_localhost() -> String {
    "localhost".to_string()
}

fn default_8086() -> i32 {
    8086
}

pub enum ReporterType {
    Log,
    Influx,
}

pub struct Reporter {
    pub reporter_type: ReporterType,
    pub influxdb_client: Option<influxdb::Client>,
}

impl Reporter {
    pub fn influxdb(&self) -> &influxdb::Client {
        let client_option = self.influxdb_client.as_ref();
        match client_option {
            None => {
                panic!("no client here")
            }
            Some(client) => {
                client
            }
        }
    }
}
