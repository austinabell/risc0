// Copyright 2023 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{rc::Rc, time::Duration};

use risc0_zkvm::{
    default_prover, sha::DIGEST_WORDS, ExecutorEnv, ExitCode, LocalProver, MemoryImage, Prover,
    ProverOpts, Receipt, Session, VerifierContext,
};
use starknet_crypto::FieldElement;

use crate::{exec_compute, get_image, Benchmark, BenchmarkAverage};

pub struct Job<'a> {
    pub spec: u32,
    pub env: ExecutorEnv<'a>,
    pub image: MemoryImage,
    pub session: Session,
    pub prover: Rc<dyn Prover>,
}

pub fn new_jobs() -> Vec<<Job<'static> as Benchmark>::Spec> {
    vec![1, 10, 100]
}

const METHOD_ID: [u32; DIGEST_WORDS] = risc0_benchmark_methods::ITER_PEDERSEN_ID;
const METHOD_PATH: &'static str = risc0_benchmark_methods::ITER_PEDERSEN_PATH;

impl Benchmark for Job<'_> {
    const NAME: &'static str = "iter_pedersen";
    type Spec = u32;
    type ComputeOut = risc0_zkvm::sha::Digest;
    type ProofType = Receipt;

    fn job_size(spec: &Self::Spec) -> u32 {
        *spec
    }

    fn output_size_bytes(_output: &Self::ComputeOut, proof: &Self::ProofType) -> u32 {
        (proof.journal.len()) as u32
    }

    fn proof_size_bytes(proof: &Self::ProofType) -> u32 {
        (proof
            .inner
            .flat()
            .unwrap()
            .iter()
            .fold(0, |acc, segment| acc + segment.get_seal_bytes().len())) as u32
    }

    fn new(spec: Self::Spec) -> Self {
        let image = get_image(METHOD_PATH);

        let mut guest_input = Vec::from([0u8; 36]);
        guest_input[0] = spec as u8;
        guest_input[1] = (spec >> 8) as u8;
        guest_input[2] = (spec >> 16) as u8;
        guest_input[3] = (spec >> 24) as u8;

        let env = ExecutorEnv::builder()
            .write(&guest_input)
            .unwrap()
            .build()
            .unwrap();

        let session = Session::new(vec![], vec![], ExitCode::Halted(0));
        let prover = Rc::new(LocalProver::new("local"));

        Job {
            spec,
            env,
            image,
            session,
            prover,
        }
    }

    fn spec(&self) -> &Self::Spec {
        &self.spec
    }

    fn host_compute(&mut self) -> Option<Self::ComputeOut> {
        let e0 = FieldElement::from_hex_be(
            "0x03d937c035c878245caf64531a5756109c53068da139362728feb561405371cb",
        )
        .unwrap();
        let e1 = FieldElement::from_hex_be(
            "0x0208a0a10250e382e1e4bbe2880906c2791bf6275695e02fbbc6aeff9cd8b31a",
        )
        .unwrap();

        let hash = starknet_crypto::pedersen_hash(&e0, &e1);

        Some(risc0_zkvm::sha::Digest::try_from(hash.to_bytes_be().as_slice()).unwrap())
    }

    fn exec_compute(&mut self) -> (u32, u32, Duration) {
        let (cycles, insn_cycles, elapsed, session) =
            exec_compute(self.image.clone(), self.env.clone());
        self.session = session;
        (cycles, insn_cycles, elapsed)
    }

    fn guest_compute(&mut self) -> (Self::ComputeOut, Self::ProofType) {
        let receipt = self.session.prove().expect("receipt");
        let result = risc0_zkvm::sha::Digest::try_from(receipt.journal.clone())
            .unwrap()
            .try_into()
            .unwrap();
        (result, receipt)
    }

    fn verify_proof(&self, _output: &Self::ComputeOut, proof: &Self::ProofType) -> bool {
        let result = proof.verify(METHOD_ID);

        match result {
            Ok(_) => true,
            Err(err) => {
                println!("{}", err);
                false
            }
        }
    }
}

impl BenchmarkAverage for Job<'_> {
    const NAME: &'static str = "pedersen";
    type Spec = u32;

    fn job_size(spec: &Self::Spec) -> u32 {
        *spec
    }

    fn new(spec: Self::Spec) -> Self {
        let image = get_image(METHOD_PATH);

        let mut guest_input = Vec::from([0u8; 36]);
        guest_input[0] = spec as u8;
        guest_input[1] = (spec >> 8) as u8;
        guest_input[2] = (spec >> 16) as u8;
        guest_input[3] = (spec >> 24) as u8;

        let env = ExecutorEnv::builder()
            .write(&guest_input)
            .unwrap()
            .build()
            .unwrap();

        let session = Session::new(vec![], vec![], ExitCode::Halted(0));
        let prover = default_prover();

        Job {
            spec,
            env,
            image,
            session,
            prover,
        }
    }

    fn spec(&self) -> &Self::Spec {
        &self.spec
    }

    fn guest_compute(&mut self) -> () {
        self.prover
            .prove(
                self.env.clone(),
                &VerifierContext::default(),
                &ProverOpts::default(),
                self.image.clone(),
            )
            .expect("receipt");
    }
}