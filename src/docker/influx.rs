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

use chrono::{DateTime, Utc};
use influxdb::InfluxDbWriteable;

use crate::docker::DockerMetrics;
use crate::report::Reporter;
use crate::util;

const INFLUX_CPU_MEASUREMENT_NAME: &str = "docker_container_cpu";
const INFLUX_MEM_MEASUREMENT_NAME: &str = "docker_container_mem";

#[derive(InfluxDbWriteable)]
struct InfluxDockerCpuMetrics {
    time: DateTime<Utc>,
    usage_in_kernelmode: u64,
    usage_in_usermode: u64,
    usage_total: u64,
    usage_system: u64,
    usage_percent: f64,
    #[influxdb(tag)] container_id: String,
    #[influxdb(tag)] container_image: String,
    #[influxdb(tag)] container_name: String,
    #[influxdb(tag)] engine_host: String,
    #[influxdb(tag)] host: String,
}

#[derive(InfluxDbWriteable)]
struct InfluxDockerMemMetrics {
    time: DateTime<Utc>,
    limit: u64,
    usage: u64,
    usage_percent: f64,
    active_anon: u64,
    active_file: u64,
    inactive_anon: u64,
    inactive_file: u64,
    pgfault: u64,
    pgmajfault: u64,
    #[influxdb(tag)] container_id: String,
    #[influxdb(tag)] container_image: String,
    #[influxdb(tag)] container_name: String,
    #[influxdb(tag)] engine_host: String,
    #[influxdb(tag)] host: String,
}

pub async fn report_influx(reporter: &Reporter, docker_metrics: Vec<DockerMetrics>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cpu_influx = vec![];
    let mut mem_influx = vec![];
    for docker_metric in docker_metrics {
        cpu_influx.push(
            InfluxDockerCpuMetrics {
                time: docker_metric.time,
                usage_in_kernelmode: docker_metric.cpu_usage_in_kernelmode,
                usage_in_usermode: docker_metric.cpu_usage_in_usermode,
                usage_total: docker_metric.cpu_total_usage,
                usage_system: docker_metric.cpu_system_usage,
                usage_percent: docker_metric.cpu_percentage,
                container_id: docker_metric.container_id.to_string(),
                container_image: docker_metric.container_image.to_string(),
                container_name: docker_metric.container_name.to_string(),
                engine_host: util::NODE_HOST_NAME.to_string(),
                host: util::NODE_HOST_NAME.to_string(),
            }.into_query(INFLUX_CPU_MEASUREMENT_NAME)
        );
        mem_influx.push(
            InfluxDockerMemMetrics {
                time: docker_metric.time,
                limit: docker_metric.memory_limit_bytes,
                usage: docker_metric.memory_usage_bytes,
                usage_percent: docker_metric.memory_percentage,
                active_anon: docker_metric.memory_active_anon,
                active_file: docker_metric.memory_active_file,
                inactive_anon: docker_metric.memory_inactive_anon,
                inactive_file: docker_metric.memory_inactive_file,
                pgfault: docker_metric.memory_pgfault,
                pgmajfault: docker_metric.memory_pgmajfault,
                container_id: docker_metric.container_id.to_string(),
                container_image: docker_metric.container_image.to_string(),
                container_name: docker_metric.container_name.to_string(),
                engine_host: util::NODE_HOST_NAME.to_string(),
                host: util::NODE_HOST_NAME.to_string(),
            }.into_query(INFLUX_MEM_MEASUREMENT_NAME)
        );
    }
    let client = reporter.influxdb();
    client.query(cpu_influx).await?;
    client.query(mem_influx).await?;
    Ok(())
}
