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

use std::collections::HashMap;
use std::time::Duration;

use bollard::container::{ListContainersOptions, MemoryStats, MemoryStatsStats, Stats, StatsOptions};
use bollard::Docker;
use chrono::{DateTime, Utc};
use futures_util::stream::StreamExt;

use lru::LruCache;

use crate::report::{Reporter, ReporterType};

mod influx;

pub async fn docker_metrics(reporter: &Reporter) {
    let mut interval_timer = tokio::time::interval(Duration::from_secs(1));
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let mut total_usage_cache: LruCache<String, u64> = LruCache::new(512);
    let mut system_usage_cache: LruCache<String, u64> = LruCache::new(512);
    loop {
        interval_timer.tick().await;
        match internal_docker_metrics(reporter, &docker, &mut total_usage_cache, &mut system_usage_cache).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("docker metrics failed {}", e);
            }
        }
    }
}

async fn internal_docker_metrics(reporter: &Reporter, docker: &Docker, total_cache: &mut LruCache<String, u64>, system_cache: &mut LruCache<String, u64>) -> Result<(), Box<dyn std::error::Error>> {
    let mut filter = HashMap::new();
    filter.insert(String::from("status"), vec![String::from("running")]);
    let containers = &docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters: filter,
            ..Default::default()
        }))
        .await?;
    if containers.is_empty() {
        return Ok(());
    }
    let time = Utc::now();
    let mut docker_metrics = vec![];
    for container in containers {
        log::trace!("container {:?} ", container);
        let container_id = container.id.as_ref().unwrap();
        let container_image = container.image.as_ref().unwrap();
        let container_name = &container.names.as_ref().unwrap()[0].trim_start_matches("/");
        let stream = &mut docker
            .stats(
                container_id,
                Some(StatsOptions {
                    stream: false,
                    ..Default::default()
                }),
            ).take(1);
        let stats_res: Result<Stats, bollard::errors::Error> = stream.next().await.unwrap();
        match stats_res {
            Ok(stats) => {
                log::trace!("container id {} name {} status {:?}", container_id, container_name, stats);
                let cpu_stats = stats.cpu_stats;
                let mem_stats = stats.memory_stats;
                log::debug!("container id {} name {} cpu status {:?}", container_id, container_name, cpu_stats);
                log::debug!("container id {} name {} mem status {:?}", container_id, container_name, mem_stats);
                let cpu_total = cpu_stats.cpu_usage.total_usage;
                let cpu_system = cpu_stats.system_cpu_usage.unwrap_or_default();
                let last_cpu_total = total_cache.get(container_id).unwrap_or(&0);
                let last_cpu_system = system_cache.get(container_id).unwrap_or(&0);
                docker_metrics.push(DockerMetrics {
                    time,
                    container_id: container_id.to_string(),
                    container_image: container_image.to_string(),
                    container_name: container_name.to_string(),
                    cpu_usage_in_kernelmode: cpu_stats.cpu_usage.usage_in_kernelmode,
                    cpu_usage_in_usermode: cpu_stats.cpu_usage.usage_in_usermode,
                    cpu_total_usage: cpu_total,
                    cpu_system_usage: cpu_system,
                    cpu_percentage: (cpu_total - last_cpu_total) as f64 / (cpu_system - last_cpu_system) as f64 * cpu_stats.online_cpus.unwrap_or_default() as f64 * 100 as f64,
                    memory_active_anon: mem_active_anon(&mem_stats),
                    memory_active_file: mem_active_file(&mem_stats),
                    memory_inactive_anon: mem_inactive_anon(&mem_stats),
                    memory_inactive_file: mem_inactive_file(&mem_stats),
                    memory_pgfault: mem_pgfault(&mem_stats),
                    memory_pgmajfault: mem_pgmajfault(&mem_stats),
                    memory_unevictable: mem_unevictable(&mem_stats),
                    memory_usage_bytes: mem_usage(&mem_stats),
                    memory_limit_bytes: mem_limit(&mem_stats),
                    memory_percentage: mem_percent(&mem_stats),
                });
                total_cache.put(String::from(container_id), cpu_total);
                system_cache.put(String::from(container_id), cpu_system);
            }
            Err(e) => {
                log::error!("{} {:?}", container_name, e);
            }
        }
    }
    report_metrics(reporter, docker_metrics).await;
    Ok(())
}

async fn report_metrics(reporter: &Reporter, docker_metrics: Vec<DockerMetrics>) {
    match reporter.reporter_type {
        ReporterType::Log => {
            for docker_metric in docker_metrics {
                log::info!("{:?}", docker_metric);
            }
        }
        ReporterType::Influx => {
            let result = influx::report_influx(reporter, docker_metrics).await;
            match result {
                Ok(_) => {}
                Err(e) => {
                    log::error!("write metrics to influx {}", e);
                }
            }
        }
    }
}

fn mem_active_anon(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.active_anon
                }
                MemoryStatsStats::V2(v2) => {
                    v2.active_anon
                }
            }
        }
    }
}

fn mem_active_file(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.active_file
                }
                MemoryStatsStats::V2(v2) => {
                    v2.active_file
                }
            }
        }
    }
}

fn mem_inactive_anon(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.inactive_anon
                }
                MemoryStatsStats::V2(v2) => {
                    v2.inactive_anon
                }
            }
        }
    }
}

fn mem_inactive_file(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.inactive_file
                }
                MemoryStatsStats::V2(v2) => {
                    v2.inactive_file
                }
            }
        }
    }
}

fn mem_pgfault(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.pgfault
                }
                MemoryStatsStats::V2(v2) => {
                    v2.pgfault
                }
            }
        }
    }
}

fn mem_pgmajfault(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.pgmajfault
                }
                MemoryStatsStats::V2(v2) => {
                    v2.pgmajfault
                }
            }
        }
    }
}

fn mem_unevictable(mem_stat: &MemoryStats) -> u64 {
    match mem_stat.stats {
        None => {
            0
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    v1.unevictable
                }
                MemoryStatsStats::V2(v2) => {
                    v2.unevictable
                }
            }
        }
    }
}

fn mem_usage(mem_stat: &MemoryStats) -> u64 {
    let v = mem_stat.usage.unwrap_or(0);
    match mem_stat.stats {
        None => {
            v
        }

        Some(stats) => {
            match stats {
                MemoryStatsStats::V1(v1) => {
                    if v1.cache > 0 {
                        v - v1.total_cache
                    } else {
                        v - v1.total_inactive_file
                    }
                }
                MemoryStatsStats::V2(v2) => {
                    v - v2.inactive_file
                }
            }
        }
    }
}

fn mem_limit(mem_stat: &MemoryStats) -> u64 {
    mem_stat.limit.unwrap_or(0)
}

fn mem_percent(mem_stat: &MemoryStats) -> f64 {
    let limit = mem_limit(mem_stat);
    if limit == 0 {
        0 as f64
    } else {
        mem_usage(mem_stat) as f64 / limit as f64
    }
}

#[derive(Debug)]
pub struct DockerMetrics {
    pub time: DateTime<Utc>,
    pub container_id: String,
    pub container_image: String,
    pub container_name: String,
    pub cpu_usage_in_kernelmode: u64,
    pub cpu_usage_in_usermode: u64,
    pub cpu_total_usage: u64,
    pub cpu_system_usage: u64,
    pub cpu_percentage: f64,
    pub memory_active_anon: u64,
    pub memory_active_file: u64,
    pub memory_inactive_anon: u64,
    pub memory_inactive_file: u64,
    pub memory_pgfault: u64,
    pub memory_pgmajfault: u64,
    pub memory_unevictable: u64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub memory_percentage: f64,
}
