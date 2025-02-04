use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use criterion::{
    black_box, criterion_group, criterion_main, measurement, BatchSize, BenchmarkGroup,
    BenchmarkId, Criterion, SamplingMode,
};

use pasta_curves::pallas;

use lurk::{
    circuit::circuit_frame::MultiFrame,
    eval::{
        empty_sym_env,
        lang::{Coproc, Lang},
    },
    field::LurkField,
    proof::nova::NovaProver,
    proof::Prover,
    ptr::Ptr,
    public_parameters::{
        instance::{Instance, Kind},
        public_params,
    },
    state::State,
    store::Store,
};

mod common;
use common::set_bench_config;

fn fib<F: LurkField>(store: &Store<F>, state: Rc<RefCell<State>>, _a: u64) -> Ptr<F> {
    let program = r#"
(letrec ((next (lambda (a b) (next b (+ a b))))
           (fib (next 0 1)))
  (fib))
"#;

    store.read_with_state(state, program).unwrap()
}

// The env output in the `fib_frame`th frame of the above, infinite Fibonacci computation will contain a binding of the
// nth Fibonacci number to `a`.
// means of computing it.]
fn fib_frame(n: usize) -> usize {
    11 + 16 * n
}

// Set the limit so the last step will be filled exactly, since Lurk currently only pads terminal/error continuations.
fn fib_limit(n: usize, rc: usize) -> usize {
    let frame = fib_frame(n);
    rc * (frame / rc + usize::from(frame % rc != 0))
}

#[derive(Clone, Debug, Copy)]
struct ProveParams {
    fib_n: usize,
    reduction_count: usize,
    date: &'static str,
    sha: &'static str,
}

impl ProveParams {
    fn name(&self) -> String {
        format!("Fibonacci-rc={}", self.reduction_count)
    }
}

fn fibo_prove<M: measurement::Measurement>(
    prove_params: ProveParams,
    c: &mut BenchmarkGroup<'_, M>,
    state: &Rc<RefCell<State>>,
) {
    let ProveParams {
        fib_n,
        reduction_count,
        date,
        sha,
    } = prove_params;

    let limit = fib_limit(fib_n, reduction_count);
    let lang_pallas = Lang::<pallas::Scalar, Coproc<pallas::Scalar>>::new();
    let lang_rc = Arc::new(lang_pallas.clone());

    // use cached public params
    let instance = Instance::new(
        reduction_count,
        lang_rc.clone(),
        true,
        Kind::NovaPublicParams,
    );
    let pp = public_params::<_, _, MultiFrame<'_, _, _>>(&instance).unwrap();

    // Track the number of `Lurk frames / sec`
    let rc = reduction_count as u64;
    c.throughput(criterion::Throughput::Elements(
        rc * u64::div_ceil((11 + 16 * fib_n) as u64, rc),
    ));

    c.bench_with_input(
        BenchmarkId::new(
            prove_params.name(),
            format!("num-{}/{sha}-{date}", prove_params.fib_n),
        ),
        &prove_params,
        |b, prove_params| {
            let store = Store::default();

            let env = empty_sym_env(&store);
            let ptr = fib::<pasta_curves::Fq>(
                &store,
                state.clone(),
                black_box(prove_params.fib_n as u64),
            );
            let prover = NovaProver::new(prove_params.reduction_count, lang_pallas.clone());

            let frames = &prover
                .get_evaluation_frames(ptr, env, &store, limit, lang_rc.clone())
                .unwrap();

            b.iter_batched(
                || (frames, lang_rc.clone()),
                |(frames, lang_rc)| {
                    let result = prover.prove(&pp, frames, &store, &lang_rc);
                    let _ = black_box(result);
                },
                BatchSize::LargeInput,
            )
        },
    );
}

fn fibonacci_prove(c: &mut Criterion) {
    set_bench_config();
    tracing::debug!("{:?}", lurk::config::LURK_CONFIG);
    let reduction_counts = [100, 600, 700, 800, 900];
    let batch_sizes = [100, 200];
    let mut group: BenchmarkGroup<'_, _> = c.benchmark_group("Prove");
    group.sampling_mode(SamplingMode::Flat); // This can take a *while*
    group.sample_size(10);
    let state = State::init_lurk_state().rccell();

    for fib_n in batch_sizes.iter() {
        for reduction_count in reduction_counts.iter() {
            let prove_params = ProveParams {
                fib_n: *fib_n,
                reduction_count: *reduction_count,
                date: env!("VERGEN_GIT_COMMIT_DATE"),
                sha: env!("VERGEN_GIT_SHA"),
            };
            fibo_prove(prove_params, &mut group, &state);
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "flamegraph")] {
        criterion_group! {
            name = benches;
            config = Criterion::default()
            .measurement_time(Duration::from_secs(120))
            .sample_size(10)
            .with_profiler(pprof::criterion::PProfProfiler::new(100, pprof::criterion::Output::Flamegraph(None)));
            targets =
             fibonacci_prove,
         }
    } else {
        criterion_group! {
            name = benches;
            config = Criterion::default()
            .measurement_time(Duration::from_secs(120))
            .sample_size(10);
            targets =
             fibonacci_prove,
         }
    }
}

criterion_main!(benches);
