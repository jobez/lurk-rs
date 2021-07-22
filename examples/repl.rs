use dendron::data::Store;
use dendron::eval::{empty_sym_env, outer_evaluate, Continuation};
use std::io::{self, BufRead, Write};

// For the moment, input must be on a single line.
fn main() {
    println!("Dendron REPL welcomes you.");

    let mut s = Store::default();
    let prompt = "> ";
    let limit = 1000;

    let stdin = io::stdin();
    let mut it = stdin.lock().lines();

    loop {
        print!("{}", prompt);
        io::stdout().flush().unwrap();

        let line = it.next().unwrap();
        let expr = s.read(&line.unwrap()).unwrap();

        println!("{}", s.print_expr(&expr.clone()));

        let (result, _next_env, limit, next_cont) =
            outer_evaluate(expr, empty_sym_env(&s), &mut s, limit);
        println!("[{} iterations] => {}", limit, s.print_expr(&result));
        match next_cont {
            Continuation::Outermost => (),
            _ => println!("Computation incomplete after limit: {}", limit),
        }
    }
}