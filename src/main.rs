use clap::{Parser, Subcommand};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use ferrosite::{
    authoring::{
        assign_slot, create_article, create_nav, create_page, create_project, edit_content,
        load_content_document, load_reorder_entries, move_reorder_entry, persist_reordered_entries,
        AssignSlotRequest, ContentDocument, EditContentRequest, NewArticleRequest, NewNavRequest,
        NewPageRequest, NewProjectRequest, ReorderEntry,
    },
    build_site, deploy_site, install_plugin, run_site, uninstall_plugin, RunOptions, SiteResult,
};

// ── CLI Definition ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "ferrosite",
    about = "Railway-oriented static site generator — powered by Rust & pfusch",
    version,
    author
)]
struct Cli {
    /// Site root directory (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the site into the output directory
    Build {
        /// Enable Puppeteer SSR pass after rendering
        #[arg(long)]
        ssr: bool,
    },

    /// Deploy the built site to the configured provider
    Deploy {
        /// Deploy provider override: cloudflare | aws | azure
        #[arg(long)]
        provider: Option<String>,

        /// Only deploy static files (skip plugin workers)
        #[arg(long)]
        static_only: bool,
    },

    /// Build and then deploy in one step
    Ship {
        /// Enable SSR pass before deploy
        #[arg(long)]
        ssr: bool,
    },

    /// Build, watch, and serve the site locally, including plugin worker routes
    Run {
        /// Host interface to bind
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind
        #[arg(short, long, default_value_t = 3000)]
        port: u16,

        /// Serve the current output directory without building or watching
        #[arg(long)]
        no_build: bool,
    },

    /// Create a new site from a template
    New {
        /// Project directory name
        name: String,

        /// Template to use (default: developer)
        #[arg(short, long, default_value = "developer")]
        template: String,

        /// Skip interactive setup and use defaults
        #[arg(long)]
        yolo: bool,
    },

    /// Validate site configuration and content without building
    Check,

    /// Create content files for articles, projects, pages, and navigation
    Add {
        #[command(subcommand)]
        command: AddCommands,
    },

    /// Update frontmatter fields on an existing content file
    Edit {
        /// Content selector: path, filename stem, slug, or exact title
        target: String,

        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        description: Option<String>,

        #[arg(long)]
        slug: Option<String>,

        #[arg(long)]
        slot: Option<String>,

        #[arg(long)]
        page_scope: Option<String>,

        #[arg(long)]
        order: Option<i32>,

        #[arg(long)]
        weight: Option<i32>,

        #[arg(long)]
        url: Option<String>,

        #[arg(long)]
        date: Option<String>,

        #[arg(long)]
        author: Option<String>,

        #[arg(long, value_delimiter = ',')]
        tags: Option<Vec<String>>,

        #[arg(long, value_delimiter = ',')]
        categories: Option<Vec<String>>,

        #[arg(long, value_delimiter = ',')]
        tech_stack: Option<Vec<String>>,

        #[arg(long)]
        repo_url: Option<String>,

        #[arg(long)]
        live_url: Option<String>,

        #[arg(long)]
        status: Option<String>,

        #[arg(long)]
        icon: Option<String>,

        #[arg(long)]
        target_page: Option<String>,

        #[arg(long)]
        body: Option<String>,

        #[arg(long)]
        open: bool,

        #[arg(long)]
        interactive: bool,
    },

    /// Change the slot and page scope for an existing content file
    AssignSlot {
        /// Content selector: path, filename stem, slug, or exact title
        target: String,

        /// Slot name, for example: hero, article-body, nav-item
        slot: String,

        #[arg(long)]
        page_scope: Option<String>,

        #[arg(long)]
        order: Option<i32>,

        #[arg(long)]
        weight: Option<i32>,
    },

    /// Interactively reorder entries within a slot and persist new order values
    Reorder {
        /// Slot to reorder, defaults to nav-item in interactive mode
        #[arg(long)]
        slot: Option<String>,

        /// Restrict reordering to a single page scope (for example: *, home, about)
        #[arg(long)]
        page_scope: Option<String>,

        /// Optional text filter applied to title, slug, or relative path
        #[arg(long)]
        query: Option<String>,

        /// First order value to assign when saving
        #[arg(long, default_value_t = 10)]
        start_at: i32,

        /// Gap between successive order values when saving
        #[arg(long, default_value_t = 10)]
        step: i32,
    },

    /// List all discovered articles and their slot assignments
    Slots,

    /// Scaffold and install SSR tooling (Node + Puppeteer)
    #[command(visible_alias = "setup-ssr")]
    SsrSetup {
        /// Package manager binary to run inside ./ssr (for example: npm, pnpm, yarn)
        #[arg(long)]
        package_manager_bin: Option<String>,
    },

    /// Print the current site config (resolved)
    Config,

    /// Install or remove plugins for the current site
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },
}

#[derive(Subcommand)]
enum PluginCommands {
    /// Install a bundled plugin or clone one from git
    #[command(visible_alias = "install")]
    Add {
        /// Bundled plugin name or git repository URL
        source: String,
    },

    /// Remove a plugin and print files that still reference it
    #[command(visible_alias = "uninstall")]
    Remove {
        /// Plugin name to remove
        plugin: String,
    },
}

#[derive(Subcommand)]
enum AddCommands {
    /// Create a blog article in content/blog/
    Article {
        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        slug: Option<String>,

        #[arg(long)]
        description: Option<String>,

        #[arg(long)]
        author: Option<String>,

        #[arg(long)]
        date: Option<String>,

        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,

        #[arg(long, value_delimiter = ',')]
        categories: Vec<String>,

        #[arg(long)]
        featured: bool,

        #[arg(long)]
        draft: bool,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        open: bool,

        #[arg(long)]
        yolo: bool,
    },

    /// Create a project case study in content/projects/
    Project {
        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        slug: Option<String>,

        #[arg(long)]
        description: Option<String>,

        #[arg(long)]
        status: Option<String>,

        #[arg(long, value_delimiter = ',')]
        tech_stack: Vec<String>,

        #[arg(long)]
        repo_url: Option<String>,

        #[arg(long)]
        live_url: Option<String>,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        open: bool,

        #[arg(long)]
        yolo: bool,
    },

    /// Create content for a page-facing section, optionally with a matching nav item
    Page {
        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        slug: Option<String>,

        #[arg(long)]
        description: Option<String>,

        #[arg(long)]
        slot: Option<String>,

        #[arg(long)]
        page_scope: Option<String>,

        #[arg(long)]
        url: Option<String>,

        #[arg(long, default_value_t = 0)]
        order: i32,

        #[arg(long, default_value_t = 50)]
        weight: i32,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        nav: bool,

        #[arg(long)]
        no_nav: bool,

        #[arg(long)]
        nav_title: Option<String>,

        #[arg(long)]
        nav_url: Option<String>,

        #[arg(long)]
        nav_icon: Option<String>,

        #[arg(long, default_value_t = 10)]
        nav_order: i32,

        #[arg(long)]
        open: bool,

        #[arg(long)]
        yolo: bool,
    },

    /// Create a standalone navigation entry
    Nav {
        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        url: Option<String>,

        #[arg(long, default_value_t = 10)]
        order: i32,

        #[arg(long, default_value_t = 50)]
        weight: i32,

        #[arg(long)]
        icon: Option<String>,

        #[arg(long)]
        external: bool,

        #[arg(long)]
        target_page: Option<String>,

        #[arg(long)]
        path: Option<PathBuf>,

        #[arg(long)]
        open: bool,

        #[arg(long)]
        yolo: bool,
    },
}

// ── Entry point ────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build { ssr } => cmd_build(&cli.root, ssr),
        Commands::Deploy {
            provider,
            static_only,
        } => cmd_deploy(&cli.root, provider, static_only),
        Commands::Ship { ssr } => cmd_ship(&cli.root, ssr),
        Commands::Run {
            host,
            port,
            no_build,
        } => cmd_run(&cli.root, &host, port, no_build),
        Commands::New {
            name,
            template,
            yolo,
        } => cmd_new(&name, &template, yolo),
        Commands::Check => cmd_check(&cli.root),
        Commands::Add { command } => cmd_add(&cli.root, command),
        Commands::Edit {
            target,
            title,
            description,
            slug,
            slot,
            page_scope,
            order,
            weight,
            url,
            date,
            author,
            tags,
            categories,
            tech_stack,
            repo_url,
            live_url,
            status,
            icon,
            target_page,
            body,
            open,
            interactive,
        } => cmd_edit(
            &cli.root,
            EditCommandArgs {
                target,
                title,
                description,
                slug,
                slot,
                page_scope,
                order,
                weight,
                url,
                date,
                author,
                tags,
                categories,
                tech_stack,
                repo_url,
                live_url,
                status,
                icon,
                target_page,
                body,
                open,
                interactive,
            },
        ),
        Commands::AssignSlot {
            target,
            slot,
            page_scope,
            order,
            weight,
        } => cmd_assign_slot(
            &cli.root,
            AssignSlotRequest {
                target,
                slot,
                page_scope,
                order,
                weight,
            },
        ),
        Commands::Reorder {
            slot,
            page_scope,
            query,
            start_at,
            step,
        } => cmd_reorder(&cli.root, slot, page_scope, query, start_at, step),
        Commands::Slots => cmd_slots(&cli.root),
        Commands::SsrSetup {
            package_manager_bin,
        } => cmd_ssr_setup(&cli.root, package_manager_bin),
        Commands::Config => cmd_config(&cli.root),
        Commands::Plugin { command } => cmd_plugin(&cli.root, command),
    };

    if let Err(e) = result {
        eprintln!("❌ Error: {}", e);
        std::process::exit(1);
    }
}

// ── Command implementations ────────────────────────────────────────────────────

fn cmd_build(root: &Path, ssr: bool) -> SiteResult<()> {
    // If --ssr flag passed, temporarily enable it
    if ssr {
        std::env::set_var("FERROSITE_SSR", "1");
    }

    let report = build_site(root)?;
    println!();
    println!("📊 Build Report");
    println!("   Pages built:        {}", report.pages_built);
    println!("   Articles processed: {}", report.articles_processed);
    println!("   Plugins loaded:     {}", report.plugins_loaded);
    println!("   SSR applied:        {}", report.ssr_applied);
    println!("   Output:             {}", report.output_dir.display());
    Ok(())
}

fn cmd_deploy(root: &Path, _provider: Option<String>, _static_only: bool) -> SiteResult<()> {
    deploy_site(root)?;
    Ok(())
}

fn cmd_ship(root: &Path, ssr: bool) -> SiteResult<()> {
    println!("🚢 Ship: building then deploying…");
    cmd_build(root, ssr)?;
    cmd_deploy(root, None, false)
}

fn cmd_run(root: &Path, host: &str, port: u16, no_build: bool) -> SiteResult<()> {
    run_site(
        root,
        &RunOptions {
            host: host.to_string(),
            port,
            no_build,
        },
    )
}

fn cmd_new(name: &str, template: &str, yolo: bool) -> SiteResult<()> {
    use ferrosite::error::SiteError;
    use std::path::Path;

    let target = Path::new(name);
    if target.exists() {
        return Err(SiteError::Build(format!(
            "Directory '{}' already exists",
            name
        )));
    }

    let resolved_template = resolve_new_site_template(template)?;
    let defaults = NewSiteAnswers::defaults(name, &resolved_template.template_name);
    let answers = if yolo {
        defaults
    } else {
        collect_new_site_answers(defaults)?
    };

    scaffold_new_site(target, &resolved_template, &answers)?;

    write_scaffold_config(target, &answers)?;

    if answers.setup_ssr {
        cmd_ssr_setup(target, None)?;
    }

    println!("✨ Created new site: {}/", name);
    println!("   Template: {}", resolved_template.display_name);
    println!();
    println!("   Next steps:");
    println!("   cd {}", name);
    println!("   # Review ferrosite.toml and content/");
    println!("   ferrosite build");
    Ok(())
}

fn cmd_check(root: &Path) -> SiteResult<()> {
    use ferrosite::pipeline::build::{build_global_slot_map, collect_articles, BuildContext};

    println!("🔍 Checking site configuration…");
    let ctx = BuildContext::load(root)?;
    println!("   ✓ Configuration valid");
    println!("   ✓ Template '{}' loaded", ctx.config.build.template);
    println!("   ✓ {} plugin(s) loaded", ctx.plugins.len());

    println!("🔍 Checking content…");
    let articles = collect_articles(&ctx)?;
    println!("   ✓ {} articles collected", articles.len());

    let global_slots = build_global_slot_map(&articles)?;
    println!("   ✓ {} global slots populated", global_slots.0.len());

    println!();
    println!("✅ All checks passed");
    Ok(())
}

#[derive(Debug, Clone)]
struct EditCommandArgs {
    target: String,
    title: Option<String>,
    description: Option<String>,
    slug: Option<String>,
    slot: Option<String>,
    page_scope: Option<String>,
    order: Option<i32>,
    weight: Option<i32>,
    url: Option<String>,
    date: Option<String>,
    author: Option<String>,
    tags: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    tech_stack: Option<Vec<String>>,
    repo_url: Option<String>,
    live_url: Option<String>,
    status: Option<String>,
    icon: Option<String>,
    target_page: Option<String>,
    body: Option<String>,
    open: bool,
    interactive: bool,
}

fn cmd_add(root: &Path, command: AddCommands) -> SiteResult<()> {
    match command {
        AddCommands::Article {
            title,
            slug,
            description,
            author,
            date,
            tags,
            categories,
            featured,
            draft,
            path,
            open,
            yolo,
        } => cmd_add_article(
            root,
            title,
            slug,
            description,
            author,
            date,
            tags,
            categories,
            featured,
            draft,
            path,
            open,
            yolo,
        ),
        AddCommands::Project {
            title,
            slug,
            description,
            status,
            tech_stack,
            repo_url,
            live_url,
            path,
            open,
            yolo,
        } => cmd_add_project(
            root,
            title,
            slug,
            description,
            status,
            tech_stack,
            repo_url,
            live_url,
            path,
            open,
            yolo,
        ),
        AddCommands::Page {
            title,
            slug,
            description,
            slot,
            page_scope,
            url,
            order,
            weight,
            path,
            nav,
            no_nav,
            nav_title,
            nav_url,
            nav_icon,
            nav_order,
            open,
            yolo,
        } => cmd_add_page(
            root,
            title,
            slug,
            description,
            slot,
            page_scope,
            url,
            order,
            weight,
            path,
            nav,
            no_nav,
            nav_title,
            nav_url,
            nav_icon,
            nav_order,
            open,
            yolo,
        ),
        AddCommands::Nav {
            title,
            url,
            order,
            weight,
            icon,
            external,
            target_page,
            path,
            open,
            yolo,
        } => cmd_add_nav(
            root,
            title,
            url,
            order,
            weight,
            icon,
            external,
            target_page,
            path,
            open,
            yolo,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_add_article(
    root: &Path,
    title: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    author: Option<String>,
    date: Option<String>,
    tags: Vec<String>,
    categories: Vec<String>,
    featured: bool,
    draft: bool,
    path: Option<PathBuf>,
    open: bool,
    yolo: bool,
) -> SiteResult<()> {
    use ferrosite::config::load_site_config_for_root;

    let config = load_site_config_for_root(root)?;
    let interactive = should_prompt(yolo);
    let title = resolve_required_input("Article title", title, interactive)?;
    let description = resolve_optional_input("Description", description, None, interactive)?;
    let author = resolve_optional_input(
        "Author",
        author,
        Some(config.site.author.name.clone()),
        interactive,
    )?;
    let date = resolve_text_input("Publish date", date, &today_iso_date(), interactive)?;
    let tags = resolve_csv_input("Tags (comma separated)", tags, interactive)?;
    let categories = resolve_csv_input("Categories (comma separated)", categories, interactive)?;

    let outcome = create_article(
        root,
        &NewArticleRequest {
            title: title.clone(),
            slug,
            description,
            author,
            date,
            tags,
            categories,
            featured,
            draft,
            path,
            body: starter_article_body(&title),
        },
    )?;

    println!("📝 Created article: {}", outcome.path.display());
    println!("   Slot: article-body");
    if open {
        open_in_editor(&outcome.path)?;
        println!("   Opened in $EDITOR");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add_project(
    root: &Path,
    title: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    status: Option<String>,
    tech_stack: Vec<String>,
    repo_url: Option<String>,
    live_url: Option<String>,
    path: Option<PathBuf>,
    open: bool,
    yolo: bool,
) -> SiteResult<()> {
    let interactive = should_prompt(yolo);
    let title = resolve_required_input("Project title", title, interactive)?;
    let description = resolve_optional_input("Description", description, None, interactive)?;
    let status = resolve_optional_input("Status", status, None, interactive)?;
    let tech_stack = resolve_csv_input("Tech stack (comma separated)", tech_stack, interactive)?;
    let repo_url = resolve_optional_input("Repository URL", repo_url, None, interactive)?;
    let live_url = resolve_optional_input("Live URL", live_url, None, interactive)?;

    let outcome = create_project(
        root,
        &NewProjectRequest {
            title: title.clone(),
            slug,
            description,
            status,
            tech_stack,
            repo_url,
            live_url,
            path,
            body: starter_project_body(&title),
        },
    )?;

    println!("🧱 Created project: {}", outcome.path.display());
    println!("   Slot: project-body");
    if open {
        open_in_editor(&outcome.path)?;
        println!("   Opened in $EDITOR");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add_page(
    root: &Path,
    title: Option<String>,
    slug: Option<String>,
    description: Option<String>,
    slot: Option<String>,
    page_scope: Option<String>,
    url: Option<String>,
    order: i32,
    weight: i32,
    path: Option<PathBuf>,
    nav: bool,
    no_nav: bool,
    nav_title: Option<String>,
    nav_url: Option<String>,
    nav_icon: Option<String>,
    nav_order: i32,
    open: bool,
    yolo: bool,
) -> SiteResult<()> {
    if nav && no_nav {
        return Err(ferrosite::error::SiteError::Build(
            "Use either --nav or --no-nav, not both.".into(),
        ));
    }

    let interactive = should_prompt(yolo);
    let title = resolve_required_input("Page title", title, interactive)?;
    let page_scope = resolve_text_input("Page scope", page_scope, "about", interactive)?;
    let default_slot = default_slot_for_scope(&page_scope);
    let slot = resolve_text_input("Slot", slot, default_slot, interactive)?;
    let description = resolve_optional_input("Description", description, None, interactive)?;
    let default_url = default_url_for_page(&page_scope, &slot, slug.as_deref(), &title);
    let url = resolve_optional_input("URL", url, Some(default_url.clone()), interactive)?;
    let create_nav_entry = if no_nav {
        false
    } else if nav {
        true
    } else if interactive {
        prompt_yes_no("Create matching nav entry?", true)?
    } else {
        true
    };

    let nav_request = if create_nav_entry {
        let nav_title = resolve_text_input("Nav title", nav_title, &title, interactive)?;
        let nav_url = resolve_text_input("Nav URL", nav_url, &default_url, interactive)?;
        let nav_icon = resolve_optional_input("Nav icon", nav_icon, None, interactive)?;

        Some(NewNavRequest {
            title: nav_title,
            url: nav_url,
            order: if interactive {
                prompt_i32_with_default("Nav order", nav_order)?
            } else {
                nav_order
            },
            weight,
            icon: nav_icon,
            external: false,
            target_page: (page_scope != "*").then(|| page_scope.clone()),
            path: None,
        })
    } else {
        None
    };

    let page_order = if interactive {
        prompt_i32_with_default("Page order", order)?
    } else {
        order
    };
    let page_weight = if interactive {
        prompt_i32_with_default("Page weight", weight)?
    } else {
        weight
    };

    let outcome = create_page(
        root,
        &NewPageRequest {
            title: title.clone(),
            slug,
            description,
            slot: slot.clone(),
            page_scope: page_scope.clone(),
            url,
            order: page_order,
            weight: page_weight,
            path,
            body: starter_page_body(&title, &slot, &page_scope),
            nav: nav_request,
        },
    )?;

    println!("📄 Created page content: {}", outcome.page_path.display());
    println!("   Slot: {}", slot);
    println!("   Scope: {}", page_scope);
    if let Some(nav_path) = &outcome.nav_path {
        println!("   Nav:   {}", nav_path.display());
    }
    if open {
        open_in_editor(&outcome.page_path)?;
        println!("   Opened in $EDITOR");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add_nav(
    root: &Path,
    title: Option<String>,
    url: Option<String>,
    order: i32,
    weight: i32,
    icon: Option<String>,
    external: bool,
    target_page: Option<String>,
    path: Option<PathBuf>,
    open: bool,
    yolo: bool,
) -> SiteResult<()> {
    let interactive = should_prompt(yolo);
    let title = resolve_required_input("Nav title", title, interactive)?;
    let url = resolve_text_input("Nav URL", url, &default_nav_url(&title), interactive)?;
    let icon = resolve_optional_input("Nav icon", icon, None, interactive)?;
    let outcome = create_nav(
        root,
        &NewNavRequest {
            title,
            url,
            order: if interactive {
                prompt_i32_with_default("Nav order", order)?
            } else {
                order
            },
            weight: if interactive {
                prompt_i32_with_default("Nav weight", weight)?
            } else {
                weight
            },
            icon,
            external,
            target_page,
            path,
        },
    )?;

    println!("🧭 Created nav entry: {}", outcome.path.display());
    if open {
        open_in_editor(&outcome.path)?;
        println!("   Opened in $EDITOR");
    }
    Ok(())
}

fn cmd_edit(root: &Path, args: EditCommandArgs) -> SiteResult<()> {
    let mut request = EditContentRequest {
        target: args.target.clone(),
        title: args.title,
        description: args.description,
        slug: args.slug,
        slot: args.slot,
        page_scope: args.page_scope,
        order: args.order,
        weight: args.weight,
        url: args.url,
        date: args.date,
        author: args.author,
        tags: args.tags.map(normalize_csv_list),
        categories: args.categories.map(normalize_csv_list),
        tech_stack: args.tech_stack.map(normalize_csv_list),
        repo_url: args.repo_url,
        live_url: args.live_url,
        status: args.status,
        icon: args.icon,
        target_page: args.target_page,
        body: args.body,
    };

    let should_interactively_edit = args.interactive
        || (!args.open && !edit_request_has_changes(&request) && io::stdin().is_terminal());

    let resolved = load_content_document(root, &args.target)?;
    if should_interactively_edit {
        request = collect_edit_request_interactively(&resolved, request)?;
    }

    let path = if edit_request_has_changes(&request) {
        let outcome = edit_content(root, &request)?;
        println!("✏️  Updated content: {}", outcome.path.display());
        outcome.path
    } else {
        resolved.path.clone()
    };

    if args.open {
        open_in_editor(&path)?;
        println!("🪄 Opened {} in $EDITOR", path.display());
    } else if !edit_request_has_changes(&request) {
        println!("ℹ️  No frontmatter changes requested.");
    }

    Ok(())
}

fn cmd_assign_slot(root: &Path, request: AssignSlotRequest) -> SiteResult<()> {
    let outcome = assign_slot(root, &request)?;
    println!("🎯 Updated slot assignment: {}", outcome.path.display());
    println!("   Slot: {}", request.slot);
    if let Some(page_scope) = request.page_scope {
        println!("   Scope: {}", page_scope);
    }
    Ok(())
}

enum ReorderAction {
    Move { from: usize, to: usize },
    Up { index: usize },
    Down { index: usize },
    Save,
    Quit,
    Help,
}

fn cmd_reorder(
    root: &Path,
    slot: Option<String>,
    page_scope: Option<String>,
    query: Option<String>,
    start_at: i32,
    step: i32,
) -> SiteResult<()> {
    if !io::stdin().is_terminal() {
        return Err(ferrosite::error::SiteError::Build(
            "Interactive reordering requires a terminal.".into(),
        ));
    }

    let slot = resolve_text_input("Slot to reorder", slot, "nav-item", true)?;
    let entries = load_reorder_entries(root, &slot, page_scope.as_deref(), query.as_deref())?;

    if entries.is_empty() {
        return Err(ferrosite::error::SiteError::Build(format!(
            "No entries found for slot '{}'{}{}.",
            slot,
            page_scope
                .as_deref()
                .map(|scope| format!(" and page_scope '{}'", scope))
                .unwrap_or_default(),
            query
                .as_deref()
                .map(|value| format!(" matching '{}'", value))
                .unwrap_or_default()
        )));
    }

    let mut entries = entries;
    println!(
        "🔀 Reordering {} entr{} in slot '{}'",
        entries.len(),
        if entries.len() == 1 { "y" } else { "ies" },
        slot
    );
    if let Some(page_scope) = &page_scope {
        println!("   Scope filter: {}", page_scope);
    }
    if let Some(query) = &query {
        println!("   Query filter: {}", query);
    }
    println!("   Commands: u <n>, d <n>, m <from> <to>, s (save), q (quit), h (help)");

    loop {
        print_reorder_entries(&entries);
        print!("reorder> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let action = parse_reorder_action(input).ok_or_else(|| {
            ferrosite::error::SiteError::Build("Unknown reorder command. Use 'h' for help.".into())
        })?;

        match action {
            ReorderAction::Move { from, to } => {
                move_reorder_entry(&mut entries, from.saturating_sub(1), to.saturating_sub(1))?;
            }
            ReorderAction::Up { index } => {
                let zero_based = index.saturating_sub(1);
                if zero_based == 0 {
                    println!("   Entry {} is already at the top.", index);
                } else {
                    move_reorder_entry(&mut entries, zero_based, zero_based - 1)?;
                }
            }
            ReorderAction::Down { index } => {
                let zero_based = index.saturating_sub(1);
                if zero_based + 1 >= entries.len() {
                    println!("   Entry {} is already at the bottom.", index);
                } else {
                    move_reorder_entry(&mut entries, zero_based, zero_based + 1)?;
                }
            }
            ReorderAction::Save => {
                let outcomes = persist_reordered_entries(root, &entries, start_at, step)?;
                println!("✅ Saved new order to {} file(s).", outcomes.len());
                return Ok(());
            }
            ReorderAction::Quit => {
                println!("ℹ️  Reordering cancelled. No files were changed.");
                return Ok(());
            }
            ReorderAction::Help => {
                println!("   u <n>       move entry n up by one");
                println!("   d <n>       move entry n down by one");
                println!("   m <a> <b>   move entry a to position b");
                println!("   s           save the new order values");
                println!("   q           quit without saving");
            }
        }
    }
}

fn print_reorder_entries(entries: &[ReorderEntry]) {
    println!();
    println!(
        "{:<4} {:<6} {:<28} {:<10} Path",
        "#", "Order", "Title", "Scope"
    );
    println!("{}", "-".repeat(90));
    for (index, entry) in entries.iter().enumerate() {
        println!(
            "{:<4} {:<6} {:<28} {:<10} {}",
            index + 1,
            entry.order,
            truncate(&entry.title, 28),
            truncate(&entry.page_scope, 10),
            entry.relative_path.display()
        );
    }
    println!();
}

fn parse_reorder_action(input: &str) -> Option<ReorderAction> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(ReorderAction::Help);
    }

    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["s"] | ["save"] => Some(ReorderAction::Save),
        ["q"] | ["quit"] => Some(ReorderAction::Quit),
        ["h"] | ["help"] => Some(ReorderAction::Help),
        ["u", index] | ["up", index] => index
            .parse::<usize>()
            .ok()
            .map(|index| ReorderAction::Up { index }),
        ["d", index] | ["down", index] => index
            .parse::<usize>()
            .ok()
            .map(|index| ReorderAction::Down { index }),
        ["m", from, to] | ["move", from, to] => {
            match (from.parse::<usize>(), to.parse::<usize>()) {
                (Ok(from), Ok(to)) => Some(ReorderAction::Move { from, to }),
                _ => None,
            }
        }
        _ => None,
    }
}

fn cmd_slots(root: &Path) -> SiteResult<()> {
    use ferrosite::pipeline::build::{collect_articles, BuildContext};

    let ctx = BuildContext::load(root)?;
    let articles = collect_articles(&ctx)?;

    println!("📋 Slot Assignments ({} articles)", articles.len());
    println!(
        "{:<40} {:<20} {:<8} {:<8} Scope",
        "Source", "Slot", "Order", "Weight"
    );
    println!("{}", "-".repeat(90));

    let mut sorted = articles.clone();
    sorted.sort_by(|a, b| {
        a.frontmatter
            .slot
            .cmp(&b.frontmatter.slot)
            .then(a.frontmatter.order.cmp(&b.frontmatter.order))
    });

    for article in &sorted {
        println!(
            "{:<40} {:<20} {:<8} {:<8} {}",
            truncate(&article.source_path, 40),
            truncate(&article.frontmatter.slot, 20),
            article.frontmatter.order,
            article.frontmatter.weight,
            article.frontmatter.page_scope,
        );
    }

    Ok(())
}

fn cmd_ssr_setup(root: &Path, package_manager_override: Option<String>) -> SiteResult<()> {
    use ferrosite::config::load_site_config;
    use ferrosite::error::SiteError;

    let ssr_dir = root.join("ssr");
    std::fs::create_dir_all(&ssr_dir).map_err(SiteError::from)?;

    // Copy the ssr/render.mjs from the crate
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ssr")
        .join("render.mjs");
    let dst = ssr_dir.join("render.mjs");

    if src.exists() {
        std::fs::copy(&src, &dst).map_err(SiteError::from)?;
    }

    // Write package.json
    let pkg = r#"{
  "name": "ferrosite-ssr",
  "type": "module",
  "dependencies": {
    "puppeteer": "^23.0.0"
  }
}"#;
    std::fs::write(ssr_dir.join("package.json"), pkg).map_err(SiteError::from)?;

    let package_manager_bin = if let Some(package_manager_bin) = package_manager_override {
        package_manager_bin
    } else {
        let config_path = root.join("ferrosite.toml");
        if config_path.exists() {
            load_site_config(&config_path)?
                .build
                .ssr
                .package_manager_bin
        } else {
            "npm".to_string()
        }
    };

    println!("📦 SSR tooling scaffolded in ./ssr/");
    println!(
        "🔧 Installing SSR dependencies with '{} install' in ./ssr/…",
        package_manager_bin
    );

    let status = Command::new(&package_manager_bin)
        .arg("install")
        .current_dir(&ssr_dir)
        .status()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => SiteError::Ssr(format!(
                "Package manager '{}' not found. Set [build.ssr].package_manager_bin or pass --package-manager-bin.",
                package_manager_bin
            )),
            _ => SiteError::from(err),
        })?;

    if !status.success() {
        return Err(SiteError::Ssr(format!(
            "'{} install' failed with status {}",
            package_manager_bin, status
        )));
    }

    persist_ssr_setup(root, &package_manager_bin)?;

    println!("   Installed with: {} install", package_manager_bin);
    println!("   Updated ferrosite.toml: [build.ssr] enabled = true");
    Ok(())
}

fn cmd_config(root: &Path) -> SiteResult<()> {
    use ferrosite::config::load_site_config_for_root;
    let config = load_site_config_for_root(root)?;
    println!("{}", toml::to_string_pretty(&config).unwrap_or_default());
    Ok(())
}

#[derive(Debug)]
struct ResolvedNewTemplate {
    template_name: String,
    display_name: String,
    source_dir: PathBuf,
    _temp_dir: Option<TempDir>,
}

fn resolve_new_site_template(template: &str) -> SiteResult<ResolvedNewTemplate> {
    use ferrosite::error::SiteError;

    if is_git_template_source(template) {
        let temp_dir = TempDir::new()?;
        let checkout_dir = temp_dir.path().join("template");
        clone_git_repo(template, &checkout_dir)?;

        let template_name = derive_template_name(template);
        return Ok(ResolvedNewTemplate {
            template_name: template_name.clone(),
            display_name: format!("{} ({})", template_name, template),
            source_dir: checkout_dir,
            _temp_dir: Some(temp_dir),
        });
    }

    let template_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join(template);

    if !template_src.exists() {
        return Err(SiteError::TemplateNotFound {
            template: template.to_string(),
        });
    }

    Ok(ResolvedNewTemplate {
        template_name: template.to_string(),
        display_name: template.to_string(),
        source_dir: template_src,
        _temp_dir: None,
    })
}

fn scaffold_new_site(
    target: &Path,
    resolved_template: &ResolvedNewTemplate,
    answers: &NewSiteAnswers,
) -> SiteResult<()> {
    copy_dir_recursive(&resolved_template.source_dir, target)?;

    if answers.template == resolved_template.template_name
        && resolved_template.display_name != resolved_template.template_name
    {
        materialize_local_template(target, &resolved_template.source_dir, &answers.template)?;
    }

    ensure_scaffold_gitignore(target)?;

    Ok(())
}

fn materialize_local_template(
    target: &Path,
    template_source: &Path,
    template_name: &str,
) -> SiteResult<()> {
    let template_root = target.join("templates").join(template_name);
    copy_dir_recursive(template_source, &template_root)?;

    let local_config = template_root.join("ferrosite.toml");
    if local_config.exists() {
        std::fs::remove_file(local_config)?;
    }

    Ok(())
}

fn cmd_plugin(root: &Path, command: PluginCommands) -> SiteResult<()> {
    match command {
        PluginCommands::Add { source } => cmd_plugin_add(root, &source),
        PluginCommands::Remove { plugin } => cmd_plugin_remove(root, &plugin),
    }
}

fn cmd_plugin_add(root: &Path, source: &str) -> SiteResult<()> {
    let outcome = install_plugin(root, source)?;

    if outcome.already_installed {
        println!("📦 Plugin '{}' is already installed", outcome.plugin_name);
    } else {
        println!("📦 Installed plugin '{}'", outcome.plugin_name);
    }

    println!("   Source: {}", outcome.source);
    println!("   Path:   {}", outcome.install_dir.display());
    println!("   Enabled in ferrosite.toml");
    Ok(())
}

fn cmd_plugin_remove(root: &Path, plugin: &str) -> SiteResult<()> {
    let outcome = uninstall_plugin(root, plugin)?;

    println!("🗑️  Removed plugin '{}'", outcome.plugin_name);
    println!("   Path: {}", outcome.removed_dir.display());

    if outcome.usage_files.is_empty() {
        println!("   No remaining usage references found.");
    } else {
        println!("⚠️  Files that still reference this plugin:");
        for path in outcome.usage_files {
            println!("   - {}", path.display());
        }
    }

    Ok(())
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len().saturating_sub(max - 1)..])
    }
}

fn is_git_template_source(source: &str) -> bool {
    source.starts_with("https://")
        || source.starts_with("ssh://")
        || source.starts_with("git@")
        || source.starts_with("file://")
        || source.ends_with(".git")
        || Path::new(source).join(".git").exists()
}

fn derive_template_name(source: &str) -> String {
    let last_segment = source
        .trim_end_matches('/')
        .rsplit(['/', '\\', ':'])
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("template")
        .trim_end_matches(".git");

    let slugged = slug::slugify(last_segment);
    if slugged.is_empty() {
        "template".to_string()
    } else {
        slugged
    }
}

fn clone_git_repo(source: &str, destination: &Path) -> SiteResult<()> {
    use ferrosite::error::SiteError;

    let status = Command::new("git")
        .arg("clone")
        .arg(source)
        .arg(destination)
        .status()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                SiteError::Build("git is required to scaffold a site from GitHub.".to_string())
            }
            _ => SiteError::from(err),
        })?;

    if !status.success() {
        return Err(SiteError::Build(format!(
            "'git clone {}' failed with status {}",
            source, status
        )));
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct NewSiteAnswers {
    directory_name: String,
    template: String,
    title: String,
    description: String,
    base_url: String,
    language: String,
    author_name: String,
    author_bio: String,
    author_avatar: String,
    github: String,
    linkedin: String,
    setup_ssr: bool,
}

impl NewSiteAnswers {
    fn defaults(name: &str, template: &str) -> Self {
        Self {
            directory_name: name.to_string(),
            template: template.to_string(),
            title: name.to_string(),
            description: "My personal site".to_string(),
            base_url: "https://example.com".to_string(),
            language: "en".to_string(),
            author_name: "Your Name".to_string(),
            author_bio: "Software developer and writer".to_string(),
            author_avatar: String::new(),
            github: "https://github.com/yourname".to_string(),
            linkedin: "https://linkedin.com/in/yourname".to_string(),
            setup_ssr: false,
        }
    }
}

fn collect_new_site_answers(defaults: NewSiteAnswers) -> SiteResult<NewSiteAnswers> {
    if !io::stdin().is_terminal() {
        return Err(ferrosite::error::SiteError::Build(
            "Interactive setup requires a terminal. Re-run with --yolo to use defaults."
                .to_string(),
        ));
    }

    println!("🧱 New site setup");
    println!("   Press enter to accept the value in brackets.");

    Ok(NewSiteAnswers {
        directory_name: defaults.directory_name,
        template: defaults.template,
        title: prompt_with_default("Website name", &defaults.title)?,
        description: prompt_with_default("Description", &defaults.description)?,
        base_url: prompt_with_default("Base URL", &defaults.base_url)?,
        language: prompt_with_default("Language", &defaults.language)?,
        author_name: prompt_with_default("Author name", &defaults.author_name)?,
        author_bio: prompt_with_default("Author bio", &defaults.author_bio)?,
        author_avatar: prompt_with_default("Author avatar", &defaults.author_avatar)?,
        github: prompt_with_default("GitHub URL", &defaults.github)?,
        linkedin: prompt_with_default("LinkedIn URL", &defaults.linkedin)?,
        setup_ssr: prompt_yes_no("Set up SSR now?", defaults.setup_ssr)?,
    })
}

fn prompt_with_default(label: &str, default: &str) -> SiteResult<String> {
    print!("{} [{}]: ", label, default);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}

fn prompt_yes_no(label: &str, default: bool) -> SiteResult<bool> {
    loop {
        let suffix = if default { "Y/n" } else { "y/N" };
        print!("{} [{}]: ", label, suffix);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_ascii_lowercase();

        match input.as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                println!("   Please answer yes or no.");
            }
        }
    }
}

fn prompt_optional_with_default(label: &str, default: &str) -> SiteResult<Option<String>> {
    let value = prompt_with_default(label, default)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn prompt_i32_with_default(label: &str, default: i32) -> SiteResult<i32> {
    loop {
        let value = prompt_with_default(label, &default.to_string())?;
        match value.trim().parse::<i32>() {
            Ok(value) => return Ok(value),
            Err(_) => println!("   Please enter a whole number."),
        }
    }
}

fn should_prompt(yolo: bool) -> bool {
    !yolo && io::stdin().is_terminal()
}

fn today_iso_date() -> String {
    chrono::Utc::now().date_naive().to_string()
}

fn resolve_required_input(
    label: &str,
    provided: Option<String>,
    interactive: bool,
) -> SiteResult<String> {
    match provided {
        Some(value) if interactive => {
            let resolved = prompt_with_default(label, &value)?;
            if resolved.trim().is_empty() {
                Err(ferrosite::error::SiteError::Build(format!(
                    "{} is required.",
                    label
                )))
            } else {
                Ok(resolved)
            }
        }
        Some(value) => Ok(value),
        None if interactive => {
            let resolved = prompt_with_default(label, "")?;
            if resolved.trim().is_empty() {
                Err(ferrosite::error::SiteError::Build(format!(
                    "{} is required.",
                    label
                )))
            } else {
                Ok(resolved)
            }
        }
        None => Err(ferrosite::error::SiteError::Build(format!(
            "{} is required when stdin is not interactive.",
            label
        ))),
    }
}

fn resolve_text_input(
    label: &str,
    provided: Option<String>,
    default: &str,
    interactive: bool,
) -> SiteResult<String> {
    if interactive {
        prompt_with_default(label, provided.as_deref().unwrap_or(default))
    } else {
        Ok(provided.unwrap_or_else(|| default.to_string()))
    }
}

fn resolve_optional_input(
    label: &str,
    provided: Option<String>,
    default: Option<String>,
    interactive: bool,
) -> SiteResult<Option<String>> {
    if interactive {
        prompt_optional_with_default(
            label,
            provided.as_deref().or(default.as_deref()).unwrap_or(""),
        )
    } else {
        Ok(provided
            .or(default)
            .and_then(|value| (!value.trim().is_empty()).then_some(value)))
    }
}

fn resolve_csv_input(
    label: &str,
    provided: Vec<String>,
    interactive: bool,
) -> SiteResult<Vec<String>> {
    let default = provided.join(", ");
    if interactive {
        let value = prompt_with_default(label, &default)?;
        Ok(parse_csv_list(&value))
    } else {
        Ok(normalize_csv_list(provided))
    }
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_csv_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .flat_map(|value| parse_csv_list(&value))
        .collect()
}

fn default_slot_for_scope(page_scope: &str) -> &'static str {
    match page_scope {
        "home" => "hero",
        "about" => "about-body",
        "contact" => "contact-form",
        "blog" => "text-block",
        "projects" => "text-block",
        _ => "text-block",
    }
}

fn default_url_for_page(page_scope: &str, slot: &str, slug: Option<&str>, title: &str) -> String {
    let slug = slug
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| slug::slugify(title));

    match (page_scope, slot) {
        ("home", _) => "/".into(),
        ("about", "about-body") => "/about/".into(),
        ("contact", "contact-form") | (_, "contact-form") => "/api/contact".into(),
        ("blog", "article-body") => format!("/blog/{}/", slug),
        ("projects", "project-body") => format!("/projects/{}/", slug),
        _ => format!("/{}/", slug),
    }
}

fn default_nav_url(title: &str) -> String {
    format!("/{}/", slug::slugify(title))
}

fn starter_article_body(title: &str) -> String {
    format!(
        "## {}\n\nWrite your article here. Add subheadings, code blocks, images, and links as needed.\n",
        title
    )
}

fn starter_project_body(title: &str) -> String {
    format!(
        "## Overview\n\nDescribe what {} solves, who it helps, and how it works.\n\n## Results\n\nAdd the technical details, screenshots, and links you want to showcase.\n",
        title
    )
}

fn starter_page_body(title: &str, slot: &str, page_scope: &str) -> String {
    match (slot, page_scope) {
        ("hero", "home") => format!(
            "Add the homepage intro for {} here. Use frontmatter fields like `headline`, `sub_headline`, `cta_label`, and `cta_url` if your template reads them.\n",
            title
        ),
        ("about-body", "about") => format!(
            "## About\n\nTell the story behind {} here. Add experience, values, and the details you want the about page to surface.\n",
            title
        ),
        ("contact-form", _) => {
            "Explain the contact flow, plugin endpoint, or extra instructions for reaching you here.\n"
                .into()
        }
        _ => format!(
            "Add the markdown body for {} here. This content will be routed by slot = \"{}\" and page_scope = \"{}\".\n",
            title, slot, page_scope
        ),
    }
}

fn open_in_editor(path: &Path) -> SiteResult<()> {
    use ferrosite::error::SiteError;

    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| {
            SiteError::Build("Set $EDITOR or $VISUAL to open files from ferrosite commands.".into())
        })?;

    let mut parts = editor.split_whitespace();
    let program = parts.next().ok_or_else(|| {
        SiteError::Build("$EDITOR is set but does not contain an executable.".into())
    })?;

    let status = Command::new(program)
        .args(parts)
        .arg(path)
        .status()
        .map_err(|err| {
            SiteError::Build(format!("Failed to launch editor '{}': {}", editor, err))
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(SiteError::Build(format!(
            "Editor '{}' exited with status {}.",
            editor, status
        )))
    }
}

fn edit_request_has_changes(request: &EditContentRequest) -> bool {
    request.title.is_some()
        || request.description.is_some()
        || request.slug.is_some()
        || request.slot.is_some()
        || request.page_scope.is_some()
        || request.order.is_some()
        || request.weight.is_some()
        || request.url.is_some()
        || request.date.is_some()
        || request.author.is_some()
        || request.tags.is_some()
        || request.categories.is_some()
        || request.tech_stack.is_some()
        || request.repo_url.is_some()
        || request.live_url.is_some()
        || request.status.is_some()
        || request.icon.is_some()
        || request.target_page.is_some()
        || request.body.is_some()
}

fn collect_edit_request_interactively(
    document: &ContentDocument,
    mut request: EditContentRequest,
) -> SiteResult<EditContentRequest> {
    let current_title = request
        .title
        .clone()
        .or_else(|| document_string(document, "title"))
        .unwrap_or_default();
    request.title = Some(prompt_with_default("Title", &current_title)?);

    let current_description = request
        .description
        .clone()
        .or_else(|| document_string(document, "description"));
    request.description =
        prompt_optional_with_default("Description", current_description.as_deref().unwrap_or(""))?;

    let current_slug = request
        .slug
        .clone()
        .or_else(|| document_string(document, "slug"));
    request.slug = prompt_optional_with_default("Slug", current_slug.as_deref().unwrap_or(""))?;

    let current_slot = request
        .slot
        .clone()
        .or_else(|| document_string(document, "slot"))
        .unwrap_or_else(|| "text-block".into());
    request.slot = Some(prompt_with_default("Slot", &current_slot)?);

    let current_scope = request
        .page_scope
        .clone()
        .or_else(|| document_string(document, "page_scope"))
        .unwrap_or_else(|| "*".into());
    request.page_scope = Some(prompt_with_default("Page scope", &current_scope)?);

    let current_url = request
        .url
        .clone()
        .or_else(|| document_string(document, "url"));
    request.url = prompt_optional_with_default("URL", current_url.as_deref().unwrap_or(""))?;

    let current_date = request
        .date
        .clone()
        .or_else(|| document_string(document, "date"));
    request.date = prompt_optional_with_default("Date", current_date.as_deref().unwrap_or(""))?;

    let current_author = request
        .author
        .clone()
        .or_else(|| document_string(document, "author"));
    request.author =
        prompt_optional_with_default("Author", current_author.as_deref().unwrap_or(""))?;

    request.order = Some(prompt_i32_with_default(
        "Order",
        request
            .order
            .unwrap_or_else(|| document_i32(document, "order").unwrap_or(0)),
    )?);
    request.weight = Some(prompt_i32_with_default(
        "Weight",
        request
            .weight
            .unwrap_or_else(|| document_i32(document, "weight").unwrap_or(50)),
    )?);

    let tags_default = request
        .tags
        .clone()
        .unwrap_or_else(|| document_string_list(document, "tags"));
    request.tags = Some(resolve_csv_input(
        "Tags (comma separated)",
        tags_default,
        true,
    )?);

    let categories_default = request
        .categories
        .clone()
        .unwrap_or_else(|| document_string_list(document, "categories"));
    request.categories = Some(resolve_csv_input(
        "Categories (comma separated)",
        categories_default,
        true,
    )?);

    let tech_stack_default = request
        .tech_stack
        .clone()
        .unwrap_or_else(|| document_string_list(document, "tech_stack"));
    request.tech_stack = Some(resolve_csv_input(
        "Tech stack (comma separated)",
        tech_stack_default,
        true,
    )?);

    let current_status = request
        .status
        .clone()
        .or_else(|| document_string(document, "status"));
    request.status =
        prompt_optional_with_default("Status", current_status.as_deref().unwrap_or(""))?;

    let current_repo_url = request
        .repo_url
        .clone()
        .or_else(|| document_string(document, "repo_url"));
    request.repo_url =
        prompt_optional_with_default("Repository URL", current_repo_url.as_deref().unwrap_or(""))?;

    let current_live_url = request
        .live_url
        .clone()
        .or_else(|| document_string(document, "live_url"));
    request.live_url =
        prompt_optional_with_default("Live URL", current_live_url.as_deref().unwrap_or(""))?;

    let current_icon = request
        .icon
        .clone()
        .or_else(|| document_string(document, "icon"));
    request.icon = prompt_optional_with_default("Icon", current_icon.as_deref().unwrap_or(""))?;

    let current_target_page = request
        .target_page
        .clone()
        .or_else(|| document_string(document, "target_page"));
    request.target_page =
        prompt_optional_with_default("Target page", current_target_page.as_deref().unwrap_or(""))?;

    Ok(request)
}

fn document_string(document: &ContentDocument, key: &str) -> Option<String> {
    document
        .frontmatter
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn document_i32(document: &ContentDocument, key: &str) -> Option<i32> {
    document
        .frontmatter
        .get(key)
        .and_then(|value| value.as_integer())
        .map(|value| value as i32)
}

fn document_string_list(document: &ContentDocument, key: &str) -> Vec<String> {
    document
        .frontmatter
        .get(key)
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn render_starter_config(answers: &NewSiteAnswers) -> String {
    format!(
        r#"[site]
title = "{title}"
description = "{description}"
base_url = "{base_url}"
language = "{language}"

[site.author]
name = "{author_name}"
bio = "{author_bio}"
avatar = "{author_avatar}"

[site.social]
github = "{github}"
linkedin = "{linkedin}"

[build]
template = "{template}"
content_dir = "content"
output_dir = "dist"
assets_dir = "assets"

[build.ssr]
enabled = false
node_bin = "node"
package_manager_bin = "npm"
timeout_ms = 30000
concurrency = 2

[layout]
menu = true
dock = false
sidebar = false

[plugins]
enabled = []

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "{directory_name}"
account_id = "YOUR_CLOUDFLARE_ACCOUNT_ID"
"#,
        title = escape_toml_basic_string(&answers.title),
        description = escape_toml_basic_string(&answers.description),
        base_url = escape_toml_basic_string(&answers.base_url),
        language = escape_toml_basic_string(&answers.language),
        author_name = escape_toml_basic_string(&answers.author_name),
        author_bio = escape_toml_basic_string(&answers.author_bio),
        author_avatar = escape_toml_basic_string(&answers.author_avatar),
        github = escape_toml_basic_string(&answers.github),
        linkedin = escape_toml_basic_string(&answers.linkedin),
        template = escape_toml_basic_string(&answers.template),
        directory_name = escape_toml_basic_string(&answers.directory_name),
    )
}

fn escape_toml_basic_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_scaffold_config(root: &Path, answers: &NewSiteAnswers) -> SiteResult<()> {
    use ferrosite::error::SiteError;

    let config_path = root.join("ferrosite.toml");
    let raw = if config_path.exists() {
        std::fs::read_to_string(&config_path).map_err(SiteError::from)?
    } else {
        render_starter_config(answers)
    };

    let mut value: toml::Value = toml::from_str(&raw).map_err(SiteError::from)?;
    let root_table = value.as_table_mut().ok_or_else(|| {
        SiteError::Config(format!(
            "'{}' must contain a top-level TOML table.",
            config_path.display()
        ))
    })?;

    let site = ensure_toml_table(root_table, "site")?;
    site.insert("title".into(), toml::Value::String(answers.title.clone()));
    site.insert(
        "description".into(),
        toml::Value::String(answers.description.clone()),
    );
    site.insert(
        "base_url".into(),
        toml::Value::String(answers.base_url.clone()),
    );
    site.insert(
        "language".into(),
        toml::Value::String(answers.language.clone()),
    );

    let author = ensure_toml_table(site, "author")?;
    author.insert(
        "name".into(),
        toml::Value::String(answers.author_name.clone()),
    );
    author.insert(
        "bio".into(),
        toml::Value::String(answers.author_bio.clone()),
    );
    author.insert(
        "avatar".into(),
        toml::Value::String(answers.author_avatar.clone()),
    );

    let social = ensure_toml_table(site, "social")?;
    social.insert("github".into(), toml::Value::String(answers.github.clone()));
    social.insert(
        "linkedin".into(),
        toml::Value::String(answers.linkedin.clone()),
    );

    let build = ensure_toml_table(root_table, "build")?;
    build.insert(
        "template".into(),
        toml::Value::String(answers.template.clone()),
    );

    if let Some(provider) = root_table
        .get("deploy")
        .and_then(|deploy| deploy.get("provider"))
        .and_then(|provider| provider.as_str())
    {
        match provider {
            "cloudflare" => {
                let deploy = ensure_toml_table(root_table, "deploy")?;
                let cloudflare = ensure_toml_table(deploy, "cloudflare")?;
                cloudflare.insert(
                    "project_name".into(),
                    toml::Value::String(answers.directory_name.clone()),
                );
            }
            "aws" => {
                let deploy = ensure_toml_table(root_table, "deploy")?;
                let aws = ensure_toml_table(deploy, "aws")?;
                aws.insert(
                    "bucket_name".into(),
                    toml::Value::String(answers.directory_name.clone()),
                );
            }
            "azure" => {
                let deploy = ensure_toml_table(root_table, "deploy")?;
                let azure = ensure_toml_table(deploy, "azure")?;
                azure.insert(
                    "app_name".into(),
                    toml::Value::String(answers.directory_name.clone()),
                );
            }
            _ => {}
        }
    }

    let rendered = toml::to_string_pretty(&value).map_err(SiteError::from)?;
    std::fs::write(&config_path, rendered).map_err(SiteError::from)?;
    Ok(())
}

fn ensure_scaffold_gitignore(root: &Path) -> SiteResult<()> {
    use ferrosite::error::SiteError;

    const CACHE_IGNORE_ENTRY: &str = ".ferrosite-cache/";

    let gitignore_path = root.join(".gitignore");
    let existing = match std::fs::read_to_string(&gitignore_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };

    let has_entry = existing
        .lines()
        .map(str::trim)
        .any(|line| line == CACHE_IGNORE_ENTRY);

    if has_entry {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(CACHE_IGNORE_ENTRY);
    updated.push('\n');

    std::fs::write(gitignore_path, updated).map_err(SiteError::from)
}

fn persist_ssr_setup(root: &Path, package_manager_bin: &str) -> SiteResult<()> {
    use ferrosite::error::SiteError;

    let config_path = root.join("ferrosite.toml");
    if !config_path.exists() {
        return Err(SiteError::Config(format!(
            "No ferrosite.toml found in '{}'. Run 'ferrosite new <name>' first or pass '--root <site-dir>'.",
            root.display()
        )));
    }

    let raw = std::fs::read_to_string(&config_path).map_err(SiteError::from)?;
    let mut value: toml::Value = toml::from_str(&raw).map_err(SiteError::from)?;
    let root_table = value.as_table_mut().ok_or_else(|| {
        SiteError::Config(format!(
            "'{}' must contain a top-level TOML table.",
            config_path.display()
        ))
    })?;

    let build = ensure_toml_table(root_table, "build")?;
    let ssr = ensure_toml_table(build, "ssr")?;
    ssr.insert("enabled".into(), toml::Value::Boolean(true));
    ssr.insert(
        "package_manager_bin".into(),
        toml::Value::String(package_manager_bin.to_string()),
    );

    let rendered = toml::to_string_pretty(&value).map_err(SiteError::from)?;
    std::fs::write(&config_path, rendered).map_err(SiteError::from)?;
    Ok(())
}

fn ensure_toml_table<'a>(
    table: &'a mut toml::map::Map<String, toml::Value>,
    key: &str,
) -> SiteResult<&'a mut toml::map::Map<String, toml::Value>> {
    use ferrosite::error::SiteError;

    let value = table
        .entry(key.to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    match value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(SiteError::Config(format!(
            "Expected '{}' in ferrosite.toml to be a table.",
            key
        ))),
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> SiteResult<()> {
    use ferrosite::error::SiteError;
    std::fs::create_dir_all(dst).map_err(SiteError::from)?;
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .map(|name| name != ".git")
                .unwrap_or(true)
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let rel = path.strip_prefix(src).map_err(SiteError::from)?;
        let dest = dst.join(rel);
        if path.is_dir() {
            std::fs::create_dir_all(&dest).map_err(SiteError::from)?;
        } else {
            std::fs::copy(path, &dest).map_err(SiteError::from)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_starter_config_uses_answers() {
        let answers = NewSiteAnswers {
            directory_name: "my-site".into(),
            template: "developer".into(),
            title: "My Site".into(),
            description: "Notes and projects".into(),
            base_url: "https://example.com".into(),
            language: "en".into(),
            author_name: "Jane Doe".into(),
            author_bio: "Builder".into(),
            author_avatar: "/avatar.jpg".into(),
            github: "https://github.com/janedoe".into(),
            linkedin: "https://linkedin.com/in/janedoe".into(),
            setup_ssr: true,
        };

        let rendered = render_starter_config(&answers);
        assert!(rendered.contains("title = \"My Site\""));
        assert!(rendered.contains("project_name = \"my-site\""));
        assert!(rendered.contains("enabled = false"));
    }

    #[test]
    fn new_site_defaults_do_not_require_a_missing_avatar_asset() {
        let defaults = NewSiteAnswers::defaults("starter", "company");

        assert_eq!(defaults.author_avatar, "");
    }

    #[test]
    fn persist_ssr_setup_enables_ssr_and_updates_package_manager() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        std::fs::write(
            root.join("ferrosite.toml"),
            r#"[site]
title = "Example"
description = "Example site"
base_url = "https://example.com"

[site.author]
name = "Example Author"

[build]
template = "developer"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#,
        )
        .expect("config");

        persist_ssr_setup(root, "pnpm").expect("persist ssr setup");

        let updated = std::fs::read_to_string(root.join("ferrosite.toml")).expect("updated config");
        let value: toml::Value = toml::from_str(&updated).expect("valid toml");

        assert_eq!(value["build"]["ssr"]["enabled"].as_bool(), Some(true));
        assert_eq!(
            value["build"]["ssr"]["package_manager_bin"].as_str(),
            Some("pnpm")
        );
    }

    #[test]
    fn write_scaffold_config_preserves_template_plugin_registration() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        std::fs::write(
            root.join("ferrosite.toml"),
            r#"[site]
title = "Horst Mustermann"
description = "Template description"
base_url = "https://horstmustermann.dev"
language = "en"
keywords = ["rust"]
favicon = "/assets/favicon.svg"

[site.author]
name = "Horst Mustermann"
email = "hello@example.com"
bio = "Template bio"
avatar = "/assets/avatar.jpg"

[site.social]
github = "https://github.com/template"
linkedin = "https://linkedin.com/in/template"
twitter = "https://x.com/template"

[build]
template = "developer"
content_dir = "content"
output_dir = "dist"
assets_dir = "assets"

[plugins]
plugins_dir = "plugins"
enabled = ["contact-form"]

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "template-project"
account_id = "abc123"
workers_subdomain = "template-subdomain"
"#,
        )
        .expect("config");

        let answers = NewSiteAnswers {
            directory_name: "my-site".into(),
            template: "developer".into(),
            title: "My Site".into(),
            description: "Notes and projects".into(),
            base_url: "https://example.com".into(),
            language: "en".into(),
            author_name: "Jane Doe".into(),
            author_bio: "Builder".into(),
            author_avatar: "/avatar.jpg".into(),
            github: "https://github.com/janedoe".into(),
            linkedin: "https://linkedin.com/in/janedoe".into(),
            setup_ssr: false,
        };

        write_scaffold_config(root, &answers).expect("write scaffold config");

        let updated = std::fs::read_to_string(root.join("ferrosite.toml")).expect("updated config");
        let value: toml::Value = toml::from_str(&updated).expect("valid toml");

        assert_eq!(value["site"]["title"].as_str(), Some("My Site"));
        assert_eq!(
            value["plugins"]["enabled"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            value["plugins"]["enabled"][0].as_str(),
            Some("contact-form")
        );
        assert_eq!(value["plugins"]["plugins_dir"].as_str(), Some("plugins"));
        assert_eq!(
            value["site"]["social"]["twitter"].as_str(),
            Some("https://x.com/template")
        );
        assert_eq!(
            value["deploy"]["cloudflare"]["project_name"].as_str(),
            Some("my-site")
        );
        assert_eq!(
            value["deploy"]["cloudflare"]["workers_subdomain"].as_str(),
            Some("template-subdomain")
        );
    }

    #[test]
    fn derive_template_name_uses_repo_basename() {
        assert_eq!(
            derive_template_name("https://github.com/example/portfolio-template.git"),
            "portfolio-template"
        );
        assert_eq!(
            derive_template_name("git@github.com:example/portfolio-template.git"),
            "portfolio-template"
        );
    }

    #[test]
    fn scaffold_new_site_from_git_template_materializes_local_template() {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo_root = temp.path().join("portfolio-template");
        std::fs::create_dir_all(repo_root.join("layouts")).expect("layouts");
        std::fs::create_dir_all(repo_root.join("components")).expect("components");
        std::fs::create_dir_all(repo_root.join("content")).expect("content");
        std::fs::create_dir_all(repo_root.join("assets")).expect("assets");
        std::fs::write(repo_root.join("layouts/base.html"), "<html></html>").expect("layout");
        std::fs::write(repo_root.join("components/card.js"), "export default {};")
            .expect("component");
        std::fs::write(repo_root.join("content/about.md"), "# About").expect("content file");
        std::fs::write(repo_root.join("assets/site.css"), "body {}").expect("asset file");
        std::fs::write(repo_root.join("theme.toml"), "[colors]\nprimary = '#000'\n")
            .expect("theme");
        std::fs::write(
            repo_root.join("ferrosite.toml"),
            r#"[site]
title = "Template Site"
description = "Template description"
base_url = "https://example.com"

[site.author]
name = "Template Author"

[build]
template = "placeholder"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "template-site"
account_id = "abc123"
"#,
        )
        .expect("config");

        let init_status = Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .current_dir(&repo_root)
            .status()
            .expect("git init");
        assert!(init_status.success());

        let email_status = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_root)
            .status()
            .expect("git config email");
        assert!(email_status.success());

        let name_status = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_root)
            .status()
            .expect("git config name");
        assert!(name_status.success());

        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .expect("git add");
        assert!(add_status.success());

        let commit_status = Command::new("git")
            .args(["commit", "-m", "Initial template"])
            .current_dir(&repo_root)
            .status()
            .expect("git commit");
        assert!(commit_status.success());

        let resolved =
            resolve_new_site_template(repo_root.to_str().expect("repo path")).expect("template");
        let target = temp.path().join("site");
        let answers = NewSiteAnswers::defaults("site", &resolved.template_name);

        scaffold_new_site(&target, &resolved, &answers).expect("scaffold");
        write_scaffold_config(&target, &answers).expect("config");

        let config = std::fs::read_to_string(target.join("ferrosite.toml")).expect("config");
        let value: toml::Value = toml::from_str(&config).expect("valid toml");

        assert_eq!(
            value["build"]["template"].as_str(),
            Some("portfolio-template")
        );
        assert!(target
            .join("templates/portfolio-template/layouts/base.html")
            .exists());
        assert!(target
            .join("templates/portfolio-template/components/card.js")
            .exists());
        assert!(target.join("content/about.md").exists());
        assert!(target.join("assets/site.css").exists());
        assert!(std::fs::read_to_string(target.join(".gitignore"))
            .expect("gitignore")
            .contains(".ferrosite-cache/"));
    }

    #[test]
    fn ensure_scaffold_gitignore_appends_cache_entry_without_clobbering() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();

        std::fs::write(root.join(".gitignore"), "dist/\nnode_modules/\n").expect("gitignore");

        ensure_scaffold_gitignore(root).expect("ensure gitignore");
        ensure_scaffold_gitignore(root).expect("ensure gitignore twice");

        let gitignore = std::fs::read_to_string(root.join(".gitignore")).expect("gitignore");
        assert!(gitignore.contains("dist/\n"));
        assert!(gitignore.contains("node_modules/\n"));
        assert_eq!(
            gitignore
                .lines()
                .filter(|line| line.trim() == ".ferrosite-cache/")
                .count(),
            1
        );
    }

    #[test]
    fn clap_parses_add_article_command() {
        let cli = Cli::try_parse_from([
            "ferrosite",
            "add",
            "article",
            "--title",
            "Launch Notes",
            "--tags",
            "launch,product",
            "--yolo",
        ])
        .expect("cli should parse");

        match cli.command {
            Commands::Add {
                command:
                    AddCommands::Article {
                        title, tags, yolo, ..
                    },
            } => {
                assert_eq!(title.as_deref(), Some("Launch Notes"));
                assert_eq!(tags, vec!["launch", "product"]);
                assert!(yolo);
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn clap_parses_assign_slot_command() {
        let cli = Cli::try_parse_from([
            "ferrosite",
            "assign-slot",
            "content/home.md",
            "hero",
            "--page-scope",
            "home",
        ])
        .expect("cli should parse");

        match cli.command {
            Commands::AssignSlot {
                target,
                slot,
                page_scope,
                ..
            } => {
                assert_eq!(target, "content/home.md");
                assert_eq!(slot, "hero");
                assert_eq!(page_scope.as_deref(), Some("home"));
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn clap_parses_reorder_command() {
        let cli = Cli::try_parse_from([
            "ferrosite",
            "reorder",
            "--slot",
            "nav-item",
            "--page-scope",
            "*",
            "--query",
            "about",
        ])
        .expect("cli should parse");

        match cli.command {
            Commands::Reorder {
                slot,
                page_scope,
                query,
                start_at,
                step,
            } => {
                assert_eq!(slot.as_deref(), Some("nav-item"));
                assert_eq!(page_scope.as_deref(), Some("*"));
                assert_eq!(query.as_deref(), Some("about"));
                assert_eq!(start_at, 10);
                assert_eq!(step, 10);
            }
            _ => panic!("unexpected command variant"),
        }
    }

    #[test]
    fn parse_reorder_action_supports_move_and_save_commands() {
        match parse_reorder_action("m 3 1") {
            Some(ReorderAction::Move { from, to }) => {
                assert_eq!(from, 3);
                assert_eq!(to, 1);
            }
            _ => panic!("expected move action"),
        }

        assert!(matches!(
            parse_reorder_action("s"),
            Some(ReorderAction::Save)
        ));
        assert!(matches!(
            parse_reorder_action("u 2"),
            Some(ReorderAction::Up { index: 2 })
        ));
    }
}
