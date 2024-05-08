use tmaze::*;

use app::{Game, GameError};

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(version, author, about, name = "tmaze")]
struct Args {
    #[clap(short, long, action, help = "Reset config to default and quit")]
    reset_config: bool,
    #[clap(short, long, action, help = "Show config path and quit")]
    show_config_path: bool,
    #[clap(long, help = "Show config in debug format and quit")]
    debug_config: bool,
}

macro_rules! testmacro {
    (eof $( $x:expr ),*) => {
        println!("eof");
    };
    ( $x:expr, $( $y:expr ),* ) => {
        println!("not eof");
    };
}

fn main() -> Result<(), GameError> {
    let _args = Args::parse();

    println!("gjklfdgh, {} {},. dfdsfg", 5, true);

    if _args.reset_config {
        settings::Settings::reset_config(settings::Settings::default_path());
        return Ok(());
    }

    if _args.show_config_path {
        if let Some(s) = settings::Settings::default_path().to_str() {
            println!("{}", s);
        } else {
            println!("{:?}", settings::Settings::default_path());
        }
        return Ok(());
    }

    if _args.debug_config {
        println!(
            "{:#?}",
            settings::Settings::load(settings::Settings::default_path())
        );
        return Ok(());
    }

    testmacro!(eof 1, 2, 3);

    Game::new().run()
}

fn f1() {
    f2()
}

fn f2() {
    f1()
}
