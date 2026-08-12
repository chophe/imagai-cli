use std::collections::HashMap;
use std::io::Read;

use clap::{Args, Parser, Subcommand};
use colored::Colorize;

use crate::config::Settings;
use crate::core::generate_image_core;
use crate::models::ImageGenerationRequest;

#[derive(Parser)]
#[command(
    name = "imagai",
    version,
    about = "🎨 Generate images using various AI APIs (OpenAI-compatible: DALL-E, GPT-image, Gemini/Imagen, Stability)."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate images from a text prompt
    Generate(Box<GenerateArgs>),
    /// List configured engines (optionally fetch available models)
    ListEngines {
        /// Query each engine and list ALL returned models (without this, only image-generation models are shown)
        #[arg(long)]
        all: bool,
    },
    /// Launch the interactive terminal UI
    Tui,
    /// Start the web interface (REST API + static UI)
    Web(WebArgs),
}

#[derive(Args, Clone)]
pub struct WebArgs {
    /// Host to bind the web server to.
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,
    /// Port to bind the web server to.
    #[arg(long, short, default_value_t = 5000)]
    pub port: u16,
}

#[derive(Args, Clone)]
pub struct GenerateArgs {
    /// The text prompt for image generation. If omitted, read from stdin.
    #[arg(short, long)]
    pub prompt: Option<String>,

    /// The image generation engine to use (e.g., openai_dalle3).
    #[arg(long)]
    pub engine: Option<String>,

    /// Output filename (e.g., my_image.png). If omitted, one is generated.
    #[arg(short, long)]
    pub output: Option<String>,

    /// Number of images to generate.
    #[arg(short = 'n', long = "num-images", default_value_t = 1)]
    pub num_images: u32,

    /// Image size (e.g., '1024x1024', '1792x1024'). Provider-dependent.
    #[arg(long, default_value = "1024x1024")]
    pub size: String,

    /// Image quality ('standard' or 'hd'). For DALL-E 3.
    #[arg(long, default_value = "standard")]
    pub quality: String,

    /// Image style ('vivid' or 'natural'). For DALL-E 3.
    #[arg(long, default_value = "vivid")]
    pub style: String,

    /// Response format ('url' or 'b64_json').
    #[arg(long, default_value = "b64_json")]
    pub response_format: String,

    /// Generate filename automatically from the prompt using an LLM.
    #[arg(long)]
    pub auto_filename: bool,

    /// Generate a random filename.
    #[arg(long)]
    pub random_filename: bool,

    /// [Stability AI] Negative prompt: what should be avoided in the image.
    #[arg(long)]
    pub negative_prompt: Option<String>,

    /// [Stability AI] Seed for reproducibility (integer).
    #[arg(long)]
    pub seed: Option<u32>,

    /// [Stability AI] Strength for image-to-image editing (0.0-1.0).
    #[arg(long)]
    pub strength: Option<f32>,

    /// [Stability AI] Output image format (e.g., 'png').
    #[arg(long)]
    pub output_format: Option<String>,

    /// [Stability AI] Aspect ratio (e.g., '1:1', '16:9'). Overrides size.
    #[arg(long)]
    pub aspect_ratio: Option<String>,

    /// [Stability AI] Generation mode: 'text-to-image' or 'image-to-image'.
    #[arg(long)]
    pub mode: Option<String>,

    /// [Vision via OpenRouter] Image URL to include for models like Gemini.
    #[arg(long)]
    pub image_url: Option<String>,

    /// Print the request body sent to the API.
    #[arg(long)]
    pub verbose: bool,
}

pub async fn run(cli: Cli, settings: &Settings) -> anyhow::Result<()> {
    match cli.command {
        Commands::Generate(args) => cmd_generate(&args, settings).await,
        Commands::ListEngines { all } => cmd_list_engines(all, settings).await,
        Commands::Tui => crate::tui::run(settings).await,
        Commands::Web(args) => crate::web::serve(settings.clone(), &args.host, args.port).await,
    }
}

async fn cmd_generate(args: &GenerateArgs, settings: &Settings) -> anyhow::Result<()> {
    // Resolve engine.
    let selected_engine = args
        .engine
        .clone()
        .or_else(|| settings.default_engine.clone());
    let Some(selected_engine) = selected_engine else {
        eprintln!(
            "{} No engine specified and no default engine configured. Use --engine or set IMAGAI__DEFAULT_ENGINE.",
            "[Error]".bold().red()
        );
        print_available_engines(settings);
        std::process::exit(1);
    };

    // Resolve prompt (read stdin if not provided).
    let prompt = match &args.prompt {
        Some(p) => p.clone(),
        None => {
            println!(
                "{} Enter your prompt. Press Ctrl+D to finish:",
                "[Prompt]".bold().yellow()
            );
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            // Strip ASCII control characters and trim.
            input
                .chars()
                .map(|c| {
                    if (c as u32) < 0x20 || c as u32 == 0x7F {
                        ' '
                    } else {
                        c
                    }
                })
                .collect::<String>()
                .trim()
                .to_string()
        }
    };
    if prompt.is_empty() {
        eprintln!("{} Prompt cannot be empty.", "[Error]".bold().red());
        std::process::exit(1);
    }

    if settings.get_engine(&selected_engine).is_none() {
        eprintln!(
            "{} Engine '{}' is not configured.",
            "[Error]".bold().red(),
            selected_engine
        );
        print_available_engines(settings);
        std::process::exit(1);
    }

    println!(
        "{} Generating image with engine: {}",
        "🖼️".bright_cyan(),
        selected_engine.bold().cyan()
    );
    println!("{} {}", "📜 Prompt:".bold(), prompt);

    let mut extra_params: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(v) = &args.negative_prompt {
        extra_params.insert("negative_prompt".into(), serde_json::json!(v));
    }
    if let Some(v) = args.seed {
        extra_params.insert("seed".into(), serde_json::json!(v));
    }
    if let Some(v) = args.strength {
        extra_params.insert("strength".into(), serde_json::json!(v));
    }
    if let Some(v) = &args.output_format {
        extra_params.insert("output_format".into(), serde_json::json!(v));
    }
    if let Some(v) = &args.aspect_ratio {
        extra_params.insert("aspect_ratio".into(), serde_json::json!(v));
    }
    if let Some(v) = &args.mode {
        extra_params.insert("mode".into(), serde_json::json!(v));
    }
    if let Some(v) = &args.image_url {
        extra_params.insert("image_url".into(), serde_json::json!(v));
    }

    let request = ImageGenerationRequest {
        prompt,
        engine: selected_engine.clone(),
        output_filename: args.output.clone(),
        size: args.size.clone(),
        quality: args.quality.clone(),
        n: args.num_images,
        style: args.style.clone(),
        response_format: args.response_format.clone(),
        extra_params,
        verbose: args.verbose,
        auto_filename: args.auto_filename,
        random_filename: args.random_filename,
    };

    println!("{} Processing...", "[spinner]".bold());
    let results = generate_image_core(&request, settings).await;

    for (i, result) in results.iter().enumerate() {
        if let Some(err) = &result.error {
            eprintln!(
                "\n{} Error generating image {}: {}",
                "[Error]".bold().red(),
                i + 1,
                err
            );
            continue;
        }
        if let Some(text) = &result.text_content {
            if result.image_url.is_none() && result.image_b64_json.is_none() {
                println!("\n{}", "✨ Model response:".bold().cyan());
                println!("{text}");
                continue;
            }
        }
        let has_payload = result.saved_path.is_some()
            || result.image_url.is_some()
            || result.image_b64_json.is_some();
        if has_payload {
            let mut msg = format!("✅ Image {} generated successfully!", i + 1);
            if let Some(path) = &result.saved_path {
                msg.push_str(&format!(" Saved to: {}", path.bold().green()));
            } else if let Some(url) = &result.image_url {
                msg.push_str(&format!(" URL: {}", url.bold().blue()));
            } else if result.image_b64_json.is_some() {
                msg.push_str(" (b64_json received, save failed or not configured)");
            }
            println!("\n{}", msg);
        } else {
            println!(
                "\n⚪ Response {} received from model (no image payload).",
                i + 1
            );
        }
        print_usage_cost(result);
    }
    Ok(())
}

async fn cmd_list_engines(all: bool, settings: &Settings) -> anyhow::Result<()> {
    if settings.engines.is_empty() {
        println!(
            "{} No engines configured. Check your .env file or environment variables.",
            "[warning]".bold().yellow()
        );
        return Ok(());
    }

    println!("{}", "⚙️ Configured Imagai Engines".bold());
    let header = format!(
        "{:<24} {:<16} {:<34} {:<28}",
        "Engine Name", "API Key Set", "Base URL", "Default Model"
    )
    .bold()
    .underline();
    println!("{header}");
    for name in settings.engine_names() {
        let cfg = &settings.engines[&name];
        let key_status = if cfg.key_set() {
            "✅ Set".green().to_string()
        } else {
            "⚠️ Not Set / Default".yellow().to_string()
        };
        let base_url = cfg
            .base_url
            .clone()
            .unwrap_or_else(|| "N/A (Official OpenAI)".to_string());
        let model = cfg
            .model
            .clone()
            .unwrap_or_else(|| "Not specified".to_string());
        println!(
            "{:<24} {:<16} {:<34} {:<28}",
            name.cyan(),
            key_status,
            base_url.green(),
            model.yellow()
        );
    }

    // Fetch models from each configured engine that has a usable key.
    for name in settings.engine_names() {
        let cfg = &settings.engines[&name];
        if !cfg.key_set() {
            println!(
                "\n{} Skipping model fetch for '{}': missing API key.",
                "[warning]".bold().yellow(),
                name
            );
            continue;
        }
        match crate::provider::fetch_models(cfg.base_url.as_deref(), &cfg.api_key).await {
            Ok(mut ids) => {
                if !all {
                    ids.retain(|m| is_image_model(m));
                }
                ids.sort();
                ids.dedup();
                println!(
                    "\n{} Models for '{}' ({})",
                    "📚".bold(),
                    name.bold().cyan(),
                    if all { "all" } else { "image-generation only" }
                );
                if ids.is_empty() {
                    println!("  {}", "[no models to display]".dimmed());
                } else {
                    for id in ids {
                        println!("  • {}", id.cyan());
                    }
                }
            }
            Err(e) => {
                println!(
                    "\n{} Failed to fetch models for '{}': {e}",
                    "[error]".bold().red(),
                    name
                );
            }
        }
    }
    Ok(())
}

fn print_available_engines(settings: &Settings) {
    let names = settings.engine_names();
    if names.is_empty() {
        eprintln!("No engines are configured. Please check your .env file.");
    } else {
        eprintln!("Available configured engines: {}", names.join(", "));
    }
}

fn print_usage_cost(result: &crate::models::ImageGenerationResponse) {
    let usage = result.usage.clone();
    let cost = result.estimated_cost.clone();
    if usage.is_none() && cost.is_none() {
        return;
    }
    println!("{}", "📊 API Usage & Cost Info".bold().magenta());
    if let Some(u) = usage {
        if let Some(obj) = u.as_object() {
            for (k, v) in obj {
                println!("  Usage.{}: {}", k, v.to_string().yellow());
            }
        } else {
            println!("  Usage: {}", u);
        }
    }
    if let Some(c) = cost {
        if let Some(obj) = c.as_object() {
            for (k, v) in obj {
                println!("  EstimatedCost.{}: {}", k, v.to_string().yellow());
            }
        } else {
            println!("  EstimatedCost: {}", c);
        }
    }
}

/// Best-effort heuristic for whether a model id is an image-generation model.
pub fn is_image_model(model_id: &str) -> bool {
    let mid = model_id.to_lowercase();
    const INDICATORS: [&str; 12] = [
        "dall-e",
        "gpt-image",
        ":image",
        "-image",
        "image-",
        "img-",
        "sd3",
        "sdxl",
        "stable",
        "stability",
        "flux",
        "imagen",
    ];
    INDICATORS.iter().any(|ind| mid.contains(ind))
}
