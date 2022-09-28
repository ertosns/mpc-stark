//! Defines an MPC fabric for the protocol
//!
//! The fabric essentially acts as a dependency injection layer. That is, the MpcFabric
//! creates and manages depedencies needed to allocate network values. This provides a
//! cleaner interface for consumers of the library; i.e. clients do not have to hold onto
//! references of the network layer or the beaver sources to allocate values.

use std::{cell::RefCell, net::SocketAddr, rc::Rc};

use async_std::task::block_on;
use curve25519_dalek::{ristretto::RistrettoPoint, scalar::Scalar};

use crate::{
    authenticated_ristretto::AuthenticatedRistretto,
    authenticated_scalar::AuthenticatedScalar,
    beaver::SharedValueSource,
    error::MpcError,
    mpc_scalar::MpcScalar,
    network::{MpcNetwork, QuicTwoPartyNet},
    BeaverSource, SharedNetwork,
};

#[derive(Clone, Debug)]
pub struct AuthenticatedMpcFabric<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> {
    /// The ID of this party in the MPC execution
    party_id: u64,
    /// The key share used to authenticate shared value openings
    key_share: MpcScalar<N, S>,
    /// The underlying network interface used to communicate between parties
    network: SharedNetwork<N>,
    /// The source from which the local party can draw results of the
    /// preprocessing functionality; i.e. Beaver triplets and shared scalars
    beaver_source: BeaverSource<S>,
}

impl<S: SharedValueSource<Scalar>> AuthenticatedMpcFabric<QuicTwoPartyNet, S> {
    /// Create a new AuthenticatedMpcFabric with the defuault (QUIC two party) network
    pub fn new(
        local_addr: SocketAddr,
        peer_addr: SocketAddr,
        beaver_source: BeaverSource<S>,
        party_id: u64,
    ) -> Result<Self, MpcError> {
        // Build the network and dial the peer
        let mut network = QuicTwoPartyNet::new(party_id, local_addr, peer_addr);
        block_on(network.connect()).map_err(MpcError::NetworkError)?;

        Ok(Self::new_with_network(
            party_id,
            Rc::new(RefCell::new(network)),
            beaver_source,
        ))
    }
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> AuthenticatedMpcFabric<N, S> {
    /// Create a new AuthenticatedMpcFabric with a specific network implementation
    pub fn new_with_network(
        party_id: u64,
        network: SharedNetwork<N>,
        beaver_source: BeaverSource<S>,
    ) -> Self {
        // Create a shared key from the beaver source
        let shared_value = beaver_source.as_ref().borrow_mut().next_shared_value();
        let key_share = MpcScalar::from_scalar_with_visibility(
            shared_value,
            crate::Visibility::Shared,
            network.clone(),
            beaver_source.clone(),
        );

        Self {
            party_id,
            key_share,
            network,
            beaver_source,
        }
    }

    /// Read the party_id field
    pub fn party_id(&self) -> u64 {
        self.party_id
    }

    /// Allocate a scalar that acts as one of the given party's private inputs to the protocol
    ///
    /// If the local party is the specified party, then this method will construct an additive sharing
    /// of the input and distribute the shares amongst the peers.
    ///
    /// If the local party is not the specified party, this method will await a share distributed by
    /// the owner of the input value.
    pub fn allocate_private_scalar(
        &self,
        owning_party: u64,
        value: Scalar,
    ) -> Result<AuthenticatedScalar<N, S>, MpcError> {
        // Create the wrapped scalar and share it
        let authenticated_value = AuthenticatedScalar::from_private_scalar(
            value,
            self.key_share.clone(),
            self.network.clone(),
            self.beaver_source.clone(),
        );

        authenticated_value
            .share_secret(owning_party)
            .map_err(MpcError::NetworkError)
    }

    /// Allocate a scalar that acts as a public value within the MPC protocol
    ///
    /// No secret shares are constructed from this, it is assumed that all parties call this method
    /// with the same (known) value
    pub fn allocate_public_scalar(&self, value: Scalar) -> AuthenticatedScalar<N, S> {
        AuthenticatedScalar::from_public_scalar(
            value,
            self.key_share.clone(),
            self.network.clone(),
            self.beaver_source.clone(),
        )
    }

    /// Allocate a RistrettoPoint that acts as one of the given party's private inputs to the protocol
    ///
    /// If the local party is the specified party, then this method will construct an additive sharing
    /// of the input and distribute the shares amongst the peers.
    ///
    /// If the local party is not the specified party, this method will await a share distributed by
    /// the owner of the input value.
    pub fn allocate_private_ristretto_point(
        &self,
        owning_party: u64,
        value: RistrettoPoint,
    ) -> Result<AuthenticatedRistretto<N, S>, MpcError> {
        let authenticated_value = AuthenticatedRistretto::from_private_ristretto_point(
            value,
            self.key_share.clone(),
            self.network.clone(),
            self.beaver_source.clone(),
        );

        authenticated_value
            .share_secret(owning_party)
            .map_err(MpcError::NetworkError)
    }

    /// Allocate a RistrettoPoint that acts as a public value within the MPC protocol
    ///
    /// No secret shares are constructed from this, it is assumed that all parties call this method
    /// with the same (known) value
    pub fn allocate_public_ristretto_point(
        &self,
        value: RistrettoPoint,
    ) -> AuthenticatedRistretto<N, S> {
        AuthenticatedRistretto::from_public_ristretto_point(
            value,
            self.key_share.clone(),
            self.network.clone(),
            self.beaver_source.clone(),
        )
    }
}
