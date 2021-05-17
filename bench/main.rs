use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::{
  collections::HashMap,
  env, fs,
  path::Path,
  process::{Command, Stdio},
  time::SystemTime,
};

mod utils;

fn read_json(filename: &str) -> Result<Value> {
  let f = fs::File::open(filename)?;
  Ok(serde_json::from_reader(f)?)
}

fn write_json(filename: &str, value: &Value) -> Result<()> {
  let f = fs::File::create(filename)?;
  serde_json::to_writer(f, value)?;
  Ok(())
}

/// The list of the examples of the benchmark name, arguments and return code
const EXEC_TIME_BENCHMARKS: &[(&str, &str, Option<i32>)] =
  &[("bench_start_time", "target/release/examples/bench_start_time", None)];

fn run_strace_benchmarks(new_data: &mut BenchResult) -> Result<()> {
  use std::io::Read;

  let mut thread_count = HashMap::<String, u64>::new();
  let mut syscall_count = HashMap::<String, u64>::new();

  for (name, example_exe, _) in EXEC_TIME_BENCHMARKS {
    let mut file = tempfile::NamedTempFile::new()?;

    println!("Starting {}", example_exe);
    Command::new("strace")
      .args(&[
        "-c",
        "-f",
        "-o",
        file.path().to_str().unwrap(),
        utils::root_path().join(example_exe).to_str().unwrap(),
      ])
      .stdout(Stdio::inherit())
      .spawn()?
      .wait()?;

    let mut output = String::new();
    file.as_file_mut().read_to_string(&mut output)?;

    let strace_result = utils::parse_strace_output(&output);
    let clone = strace_result.get("clone").map(|d| d.calls).unwrap_or(0) + 1;
    let total = strace_result.get("total").unwrap().calls;
    thread_count.insert(name.to_string(), clone);
    syscall_count.insert(name.to_string(), total);
  }

  new_data.thread_count = thread_count;
  new_data.syscall_count = syscall_count;

  Ok(())
}

fn run_max_mem_benchmark() -> Result<HashMap<String, u64>> {
  let mut results = HashMap::<String, u64>::new();

  for (name, example_exe, return_code) in EXEC_TIME_BENCHMARKS {
    let proc = Command::new("time")
      .args(&["-v", utils::root_path().join(example_exe).to_str().unwrap()])
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()?;

    let proc_result = proc.wait_with_output()?;
    if let Some(code) = return_code {
      assert_eq!(proc_result.status.code().unwrap(), *code);
    }
    let out = String::from_utf8(proc_result.stderr)?;

    results.insert(name.to_string(), utils::parse_max_mem(&out).unwrap());
  }

  Ok(results)
}

fn rlib_size(target_dir: &std::path::Path, prefix: &str) -> u64 {
  let mut size = 0;
  let mut seen = std::collections::HashSet::new();
  for entry in std::fs::read_dir(target_dir.join("deps")).unwrap() {
    let entry = entry.unwrap();
    let os_str = entry.file_name();
    let name = os_str.to_str().unwrap();
    if name.starts_with(prefix) && name.ends_with(".rlib") {
      let start = name.split('-').next().unwrap().to_string();
      if seen.contains(&start) {
        println!("skip {}", name);
      } else {
        seen.insert(start);
        size += entry.metadata().unwrap().len();
        println!("check size {} {}", name, size);
      }
    }
  }
  assert!(size > 0);
  size
}

fn get_binary_sizes(target_dir: &Path) -> Result<HashMap<String, u64>> {
  let mut sizes = HashMap::<String, u64>::new();

  let wry_size = rlib_size(&target_dir, "libwry");
  println!("wry {} bytes", wry_size);
  sizes.insert("wry_rlib".to_string(), wry_size);

  // add up size for everything in target/release/deps/libtao*
  let tao_size = rlib_size(&target_dir, "libtao");
  println!("tao {} bytes", tao_size);
  sizes.insert("tao_rlib".to_string(), tao_size);

  Ok(sizes)
}

fn cargo_deps() -> usize {
  let cargo_lock = utils::root_path().join("Cargo.lock");
  let mut count = 0;
  let file = std::fs::File::open(cargo_lock).unwrap();
  use std::io::BufRead;
  for line in std::io::BufReader::new(file).lines() {
    if line.unwrap().starts_with("[[package]]") {
      count += 1
    }
  }
  println!("cargo_deps {}", count);
  assert!(count > 10); // Sanity check.
  count
}

#[derive(Default, Serialize, Debug)]
struct BenchResult {
  created_at: String,
  sha1: String,

  //exec_time: HashMap<String, HashMap<String, f64>>,
  binary_size: HashMap<String, u64>,
  max_memory: HashMap<String, u64>,
  thread_count: HashMap<String, u64>,
  syscall_count: HashMap<String, u64>,
  cargo_deps: usize,
  //max_latency: HashMap<String, f64>,
  //lsp_exec_time: HashMap<String, u64>,
  //req_per_sec: HashMap<String, u64>,
  //throughput: HashMap<String, f64>,
}

fn main() -> Result<()> {
  if env::args().find(|s| s == "--bench").is_none() {
    return Ok(());
  }

  println!("Starting wry benchmark");

  let target_dir = utils::target_dir();
  env::set_current_dir(&utils::root_path())?;

  let mut new_data = BenchResult {
    created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    sha1: utils::run_collect(&["git", "rev-parse", "HEAD"], None, None, None, true)
      .0
      .trim()
      .to_string(),
    //exec_time: run_exec_time(&deno_exe, &target_dir)?,
    binary_size: get_binary_sizes(&target_dir)?,
    //bundle_size: bundle_benchmark(&wry_exe)?,
    cargo_deps: cargo_deps(),
    //lsp_exec_time: lsp::benchmarks(&deno_exe)?,
    ..Default::default()
  };

  if cfg!(target_os = "linux") {
    run_strace_benchmarks(&mut new_data)?;
    new_data.max_memory = run_max_mem_benchmark()?;
  }

  println!("===== <BENCHMARK RESULTS>");
  serde_json::to_writer_pretty(std::io::stdout(), &new_data)?;
  println!("\n===== </BENCHMARK RESULTS>");

  if let Some(filename) = target_dir.join("bench.json").to_str() {
    write_json(filename, &serde_json::to_value(&new_data)?)?;
  } else {
    eprintln!("Cannot write bench.json, path is invalid");
  }

  Ok(())
}
