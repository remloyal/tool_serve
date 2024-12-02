use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::panic;
use std::process::Command;

use bytes::Buf;
use futures_util::stream::TryStreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::{self, File};
use std::net::{SocketAddr, TcpListener};
use std::path::Path;
use std::result::Result;
use warp::filters::multipart::FormData;
use warp::Filter;
// use std::io::copy;
// use tokio::io::AsyncWriteExt;
// use warp::http::StatusCode;
// use warp::reject::custom;
// use warp::reply::with_status;

// 定义请求体的结构
#[derive(Deserialize, Serialize, Debug)]
struct RequestBody {
    path: String,
}

fn find_available_port(start: u16, end: u16) -> Option<u16> {
    for port in start..=end {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if TcpListener::bind(&addr).is_ok() {
            return Some(port); // 找到可用的端口
        }
    }
    None // 所有端口都被占用
}

#[tokio::main]
async fn main() {
    let start_port = 54321;
    let end_port = 54421;
    let mut web_port = 54321;
    match find_available_port(start_port, end_port) {
        Some(port) => {
            web_port = port;
            println!("找到可用的端口: {}", port)
        }
        None => println!("在区间 {} 到 {} 内没有可用的端口", start_port, end_port),
    }
    // let mut url = String::from("127.0.0.1:");
    // url.push_str(&web_port.to_string());

    // 设置默认路由
    let default_route = warp::any().map(|| warp::reply::html("Welcome to tool"));

    let json_route = warp::path("json").map(|| {
        let response = json!({
            "message": "Hello, World!",
            "status": "success"
        });
        warp::reply::json(&response)
    });

    let open_file_router = warp::path("open")
        .and(warp::get())
        .and(warp::query::<HashMap<String, String>>())
        .map(|query: HashMap<String, String>| {
            match query.get("path") {
                Some(ref path) => {
                    // 去掉字符串的引号
                    let clean_path = path.trim_matches('"'); // 去掉首尾的双引号
                    println!("{:?}", clean_path);
                    Command::new("sh")
                        .args(&["-c", &format!("cd ~/ && open  {}", &clean_path)])
                        .output()
                        .expect("Failed to execute command");

                    warp::reply::json(&json!({
                        "code":0,
                        "status": "success",
                    }))
                }
                None => warp::reply::json(&json!({
                    "status": "error",
                    "message": "路径错误",
                    "query":query,
                })),
            }
        });
    let post_filter = warp::path("open")
        .and(warp::post())
        .and(warp::body::json()) // 解析请求体为JSON
        .map(|body: RequestBody| {
            println!("Received POST request with name: {},", body.path);
            Command::new("sh")
                .args(&["-c", &format!("cd ~/ && open  {}", &body.path)])
                .output()
                .expect("Failed to execute command");
            warp::reply::json(&json!({
                "code":0,
                "status": "success",
                "body":body
            }))
        });

    // 定义一个路由，用于判断文件或目录，使用查询参数
    let file_route = warp::path("file")
        .and(warp::query::<HashMap<String, String>>()) // 获取查询参数
        .map(|query: HashMap<String, String>| {
            match query.get("path") {
                Some(ref path) => {
                    // 去掉字符串的引号
                    let clean_path = path.trim_matches('"'); // 去掉首尾的双引号
                    println!("{:?}", clean_path);

                    match check_path_type(&clean_path, 0) {
                        Ok(content) => {
                            // println!("{:?}", content);
                            warp::reply::json(&json!({
                                "code":0,
                                "status": "success",
                                "data": content
                            }))
                        }
                        Err(err) => warp::reply::json(&json!({
                            "code":404,
                            "status": "error",
                            "message": "无法读取文件",
                            "data":query,
                            "error":err.to_string(),
                        })),
                    }
                }
                None => warp::reply::json(&json!({
                    "status": "error",
                    "message": "路径错误",
                    "data":query,
                })),
            }
        });

    // 保存文件 saveFile
    // 定义处理上传请求的过滤器
    let upload_route = warp::path("saveFile")
        .and(warp::post())
        .and(warp::multipart::form())
        .and_then(handle_file_upload1);
    // 启动服务器
    let routes = json_route
        .or(file_route)
        .or(upload_route)
        .or(open_file_router)
        .or(post_filter)
        .or(default_route);
    warp::serve(routes).run(([127, 0, 0, 1], web_port)).await;
}

fn check_path_type(path: &str, limit: i32) -> Result<HashMap<String, String>, String> {
    let new_path = Path::new(path);
    let metadata_result = fs::metadata(new_path);
    match metadata_result {
        Ok(metadata) => {
            if metadata.is_dir() {
                // println!("{} 是一个文件夹。", path);
                if limit == 0 {
                    let data = list_files_in_directory(path)?;
                    Ok(data)
                } else {
                    println!("{} Nesting Query", path);
                    Ok(HashMap::new())
                }
            } else if metadata.is_file() {
                // println!("{} 是一个文件。", path);
                let mut map: HashMap<String, String> = HashMap::new();
                let data = read_type_file(path)?;
                map.insert(path.to_string(), data);
                Ok(map)
            } else {
                // println!("{} 是其他类型的文件（例如符号链接）。", path);
                Ok(HashMap::new())
            }
        }
        Err(e) => {
            println!("无法获取元数据: {} - 错误信息: {}", path, e);
            Err(e.to_string())
        }
    }
}

fn list_files_in_directory(dir_path: &str) -> Result<HashMap<String, String>, String> {
    let entries = fs::read_dir(dir_path).map_err(|e| e.to_string())?;
    let mut map: HashMap<String, String> = HashMap::new();

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            println!("文件: {}", path.display());
            let data =
                read_type_file(path.to_str().unwrap()).map_err(|_| "读取文件失败".to_string())?;
            // map.insert(path.to_string_lossy().to_string(), data);
            // 获取文件名
            match path.file_name() {
                Some(file_name) => {
                    // 转换为字符串
                    if let Some(file_name_str) = file_name.to_str() {
                        map.insert(file_name_str.to_owned(), data);
                    } else {
                        map.insert(path.to_string_lossy().to_string(), data);
                    }
                }
                None => {
                    map.insert(path.to_string_lossy().to_string(), data);
                }
            }
        } else if path.is_dir() {
            println!("目录: {}", path.display());
            let subdir_result = check_path_type(path.to_string_lossy().to_string().as_str(), 1);
            if let Err(e) = subdir_result {
                return Err(e);
            }
            let subdir_map = subdir_result.unwrap();
            for (k, v) in subdir_map {
                map.insert(k, v);
            }
        }
    }
    // 移除值为空字符串的条目
    map.retain(|_, value| !value.is_empty());
    // 移除不包含 .pdf 或 .csv 的键
    map.retain(|key, _| key.ends_with(".pdf") || key.ends_with(".csv"));
    Ok(map)
}

fn read_type_file(path: &str) -> Result<String, String> {
    let file_path = Path::new(path);
    if let Some(extension) = file_path.extension() {
        let ext_lower = extension.to_str().unwrap_or("").to_lowercase();
        match ext_lower.as_str() {
            "pdf" => {
                // println!("{} 是一个 PDF 文件。", file_path.display());
                match read_pdf(path) {
                    Ok(data) => Ok(data),
                    Err(_) => {
                        println!(
                            "{}",
                            format!("读取 PDF 文件 {} 失败。", file_path.display())
                        );
                        Ok("".to_string())
                    }
                }
                // if let Ok(data) = read_pdf(path) {
                //     Ok(data)
                // } else {
                //     println!(
                //         "{}",
                //         format!("读取 PDF 文件 {} 失败。", file_path.display())
                //     );
                //     Ok("".to_string())
                // }
            }
            "csv" => {
                // println!("{} 是一个 CSV 文件。", file_path.display());
                if let Ok(content) = read_csv_to_string(path) {
                    Ok(content)
                } else {
                    println!(
                        "{}",
                        format!("读取 CSV 文件 {} 失败。", file_path.display())
                    );
                    Ok("".to_string())
                }
            }
            _ => {
                // println!("{} 不是 PDF 或 CSV 文件。", file_path.display());
                // Err(format!("{} 不是 PDF 或 CSV 文件。", file_path.display()))
                Ok("".to_string())
            }
        }
    } else {
        println!("{} 没有扩展名。", file_path.display());
        if let Ok(content) = read_csv_to_string(path) {
            Ok(content)
        } else {
            // Err(format!("读取 没有扩展名 文件 {} 失败。", file_path.display()))
            Ok("".to_string())
        }
        // Err(format!("{} 没有扩展名。", file_path.display()))
    }
}

fn read_pdf(path: &str) -> Result<String, String> {
    let result = panic::catch_unwind(|| {
        pdf_extract::extract_text(path)
    });
    match result {
        Ok(Ok(s)) => Ok(s),
        Ok(Err(err)) => {
            println!("Err: {}", err);
            Ok("".to_string())
        },
        Err(_) => {
            println!("Panic occurred while extracting text from PDF.");
            Ok("".to_string())
        }
    }
}

fn read_csv_to_string(path: &str) -> Result<String, Box<dyn Error>> {
    let mut file = File::open(path)?; // 打开文件
    let mut contents = String::new(); // 创建一个空字符串
    file.read_to_string(&mut contents)?; // 读取文件内容到字符串中
    Ok(contents) // 返回字符串内容
}

async fn handle_file_upload1(mut form: FormData) -> Result<impl warp::Reply, warp::Rejection> {
    let mut file_path: Option<String> = None;

    // let mut fields: Vec<(String, String)> = Vec::new();
    let mut file_contents: Vec<u8> = Vec::new();

    while let Some(mut field) = form.try_next().await.map_err(|_| warp::reject())? {
        // 处理 file 字段
        if field.name() == "file" {
            // 读取文件内容
            while let Some(chunk) = field.data().await {
                let bytes = chunk.map_err(|_| warp::reject())?;
                // 这里我们将 bytes 转换为 &[u8]
                file_contents.extend_from_slice(bytes.chunk()); // 使用 as_ref() 将 impl Buf 转换为 &[u8]
            }
        } else if field.name() == "filePath" {
            let path = field.data().await.ok_or_else(warp::reject)?;
            let bytes = path.map_err(|_| warp::reject())?;
            file_path = Some(String::from_utf8_lossy(&bytes.chunk()).to_string());
        }
    }
    let save_path = file_path.ok_or_else(|| warp::reject())?;

    // 保存文件
    println!("保存路径: {} ", save_path);
    let mut file = File::create(save_path).map_err(|_| warp::reject())?;
    file.write_all(&file_contents).map_err(|_| warp::reject())?;
    Ok(format!("File saved successfully!"))
}
