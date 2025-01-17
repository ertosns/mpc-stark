#![deny(unsafe_code)]
#![deny(missing_docs)]
#![deny(clippy::missing_docs_in_private_items)]
#![allow(incomplete_features)]
#![feature(inherent_associated_types)]

//! Defines an MPC implementation over the Stark curve that allows for out-of-order execution of
//! the underlying MPC circuit

use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

use algebra::{scalar::Scalar, stark_curve::StarkPoint};
use beaver::SharedValueSource;

use network::MpcNetwork;
use rand::thread_rng;

pub mod algebra;
pub mod beaver;
#[cfg(feature = "benchmarks")]
pub mod buffer;
#[cfg(not(feature = "benchmarks"))]
pub(crate) mod buffer;
pub mod commitment;
pub mod error;
mod fabric;
#[cfg(feature = "benchmarks")]
pub use fabric::*;
#[cfg(not(feature = "benchmarks"))]
pub use fabric::{FabricInner, MpcFabric, ResultHandle, ResultId, ResultValue};
pub mod network;

// -------------
// | Constants |
// -------------

/// The first party
pub const PARTY0: u64 = 0;
/// The second party
pub const PARTY1: u64 = 1;

/// Generate a random curve point by multiplying a random scalar with the
/// Stark curve group generator
pub fn random_point() -> StarkPoint {
    let mut rng = thread_rng();
    StarkPoint::generator() * Scalar::random(&mut rng)
}

// --------------------
// | Crate-wide Types |
// --------------------

/// A type alias for a shared locked value
type Shared<T> = Arc<RwLock<T>>;

/// SharedNetwork wraps a network implementation in a borrow-safe container
/// while providing interior mutability
#[allow(type_alias_bounds)]
pub type SharedNetwork<N: MpcNetwork + Send> = Rc<RefCell<N>>;
/// A type alias for a shared, mutable reference to an underlying beaver source
#[allow(type_alias_bounds)]
pub type BeaverSource<S: SharedValueSource> = Rc<RefCell<S>>;

#[cfg(test)]
pub(crate) mod test_helpers {
    use futures::Future;

    use crate::{
        beaver::PartyIDBeaverSource,
        network::{MockNetwork, NoRecvNetwork, UnboundedDuplexStream},
        MpcFabric, PARTY0, PARTY1,
    };

    /// Create a mock fabric
    pub fn mock_fabric() -> MpcFabric {
        let network = NoRecvNetwork::default();
        let beaver_source = PartyIDBeaverSource::default();

        MpcFabric::new(network, beaver_source)
    }

    /// Run a mock MPC connected by a duplex stream as the mock network
    ///
    /// This will spawn two tasks to execute either side of the MPC
    ///
    /// Returns the outputs of both parties
    pub async fn execute_mock_mpc<T, S, F>(mut f: F) -> (T, T)
    where
        T: Send + 'static,
        S: Future<Output = T> + Send + 'static,
        F: FnMut(MpcFabric) -> S,
    {
        // Build a duplex stream to broker communication between the two parties
        let (party0_stream, party1_stream) = UnboundedDuplexStream::new_duplex_pair();
        let party0_fabric = MpcFabric::new(
            MockNetwork::new(PARTY0, party0_stream),
            PartyIDBeaverSource::new(PARTY0),
        );
        let party1_fabric = MpcFabric::new(
            MockNetwork::new(PARTY1, party1_stream),
            PartyIDBeaverSource::new(PARTY1),
        );

        // Spawn two tasks to execute the MPC
        let fabric0 = party0_fabric.clone();
        let fabric1 = party1_fabric.clone();
        let party0_task = tokio::spawn(f(fabric0));
        let party1_task = tokio::spawn(f(fabric1));

        let party0_output = party0_task.await.unwrap();
        let party1_output = party1_task.await.unwrap();

        // Shutdown the fabrics
        party0_fabric.shutdown();
        party1_fabric.shutdown();

        (party0_output, party1_output)
    }
}
