extern crate tiny_http;

use std::fs::{DirEntry, File};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use std::{thread, vec};

use gumdrop::Options;
use prometheus_client::encoding::text::encode;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;

const DAQS_DIR: &str = "/mnt/ffs/data/daqs";

// csv relative columns
const COL_VIN: usize = 0;
#[allow(dead_code)]
const COL_IIN: usize = 1;
const COL_TEMP: usize = 2;
#[allow(dead_code)]
const COL_PWM: usize = 3;
#[allow(dead_code)]
const COL_STATUS: usize = 4;
#[allow(dead_code)]
const COL_FLAGS: usize = 5;
const COL_RSSI: usize = 6;
#[allow(dead_code)]
const COL_BRSSI: usize = 7;
#[allow(dead_code)]
const COL_ID: usize = 8;
#[allow(dead_code)]
const COL_VOUT: usize = 9;
#[allow(dead_code)]
const COL_DETAILS: usize = 10;
const COL_PIN: usize = 11;

#[derive(Debug, Options)]
struct MyOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(free)]
    tigo_daqs_data_dir: String,

    #[options(help = "bind ip: default(0.0.0.0)")]
    bind_ip: Option<String>,

    #[options(help = "bind port: default(9980)")]
    bind_port: Option<u16>,

    #[options(help = "verbose output")]
    verbose: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
struct Labels {
    // modul name
    name: String,
}

fn update_gauge(
    gauge: &Family<Labels, Gauge<f64, AtomicU64>>,
    module_index: usize,
    value_opt: Option<f64>,
) {
    let current_label = &Labels {
        name: format!("A{}", module_index),
    };
    match value_opt {
        Some(value) => gauge.get_or_create(current_label).set(value),
        None => {
            gauge.remove(current_label);
            0.0
        }
    };
}

fn get_newest_csv_file(data_dir: &str) -> Option<DirEntry> {
    let last_modified_file = std::fs::read_dir(data_dir)
        .expect("Couldn't access daqs directory")
        .flatten() // Remove failed
        .filter(|f| {
            f.metadata().unwrap().is_file() && f.file_name().to_str().unwrap().ends_with(".csv")
        }) // Filter out directories (only consider csv files)
        .max_by_key(|x| x.metadata().unwrap().modified().unwrap()); // Get the most recently modified file

    last_modified_file
}

fn get_field_value(field: Option<&str>) -> Option<f64> {
    let s = field.unwrap();
    if s.is_empty() {
        None
    } else {
        Some(s.parse::<f64>().unwrap())
    }
}

fn main() {
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        std::process::exit(1);
    }));

    let opts = MyOptions::parse_args_default_or_exit();
    let daqs_data_dir = if opts.tigo_daqs_data_dir.is_empty() {
        DAQS_DIR.to_string()
    } else {
        opts.tigo_daqs_data_dir
    };

    // prometheus registry
    let mut registry = <Registry>::default();
    let module_power = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let module_volts = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let module_rssi = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let module_temp = Family::<Labels, Gauge<f64, AtomicU64>>::default();
    let tigo_timestamp = Family::<Vec<(String, String)>, Gauge<f64, AtomicU64>>::default();
    registry.register(
        "tigo_module_power",
        "Modul power value in W",
        module_power.clone(),
    );
    registry.register(
        "tigo_module_volts",
        "Modul volt value in V",
        module_volts.clone(),
    );
    registry.register(
        "tigo_module_rssi",
        "Tigo signal strength value",
        module_rssi.clone(),
    );
    registry.register(
        "tigo_module_temp",
        "Tigo module temperature value in celsius",
        module_temp.clone(),
    );
    registry.register(
        "tigo_timestamp",
        "timestamp of the dataset",
        tigo_timestamp.clone(),
    );

    let timestamp_label = vec![("local".to_string(), "cca".to_string())];

    let http_client_handle = thread::spawn(move || loop {
        let current_csv_opt = get_newest_csv_file(daqs_data_dir.as_str());

        if current_csv_opt.is_none() {
            panic!("No current file found in {}", daqs_data_dir);
        }

        let input_res = File::open(current_csv_opt.unwrap().path());
        match input_res {
            Ok(input) => {
                // Build the CSV reader and iterate over each record.
                let mut rdr = csv::Reader::from_reader(input);

                match rdr.headers() {
                    Ok(headers) => {
                        let module_count = (headers.len() - 3) / 12;
                        // println!("modules: {}", (headers.len() - 3) / 12);

                        let last_opt = rdr.records().last();
                        if last_opt.as_ref().is_some_and(|x| x.is_ok()) {
                            let last = last_opt.unwrap().unwrap();

                            for i in 0..module_count {
                                let start_index = 3 + i * 12;
                                let module_index = i + 1;

                                let vin = get_field_value(last.get(start_index + COL_VIN));
                                let rssi = get_field_value(last.get(start_index + COL_RSSI));
                                let pin = get_field_value(last.get(start_index + COL_PIN));
                                let temp = get_field_value(last.get(start_index + COL_TEMP));

                                // println!("A{}: {:?}/{:?}/{:?}", module_index, vin, rssi, pin);

                                update_gauge(&module_volts, module_index, vin);
                                update_gauge(&module_rssi, module_index, rssi);
                                update_gauge(&module_power, module_index, pin);
                                update_gauge(&module_temp, module_index, temp);
                            }

                            let last_timestamp = get_field_value(last.get(1));
                            // println!("last update: {}", last_timestamp.unwrap());
                            tigo_timestamp
                                .get_or_create(&timestamp_label)
                                .set(last_timestamp.unwrap());
                        }
                    }
                    Err(_) => println!("csv file doesn't have a header"),
                }
            }
            Err(err) => println!("Unable to open file: {}", err),
        }

        std::thread::sleep(Duration::from_secs(10));
    });

    let bind_address = format!(
        "{}:{}",
        opts.bind_ip.unwrap_or("0.0.0.0".to_string()),
        opts.bind_port.unwrap_or(9980)
    );
    let server = Arc::new(tiny_http::Server::http(&bind_address).unwrap());
    println!("Now listening on {}", bind_address);

    for rq in server.incoming_requests() {
        let mut buffer = String::new();
        encode(&mut buffer, &registry).unwrap();
        let response = tiny_http::Response::from_string(buffer);
        let _ = rq.respond(response);
    }

    http_client_handle.join().unwrap();
}
