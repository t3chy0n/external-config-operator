use futures::join;
use crate::controller::utils::context::Data;
use crate::controller::controller::{run as startController};

use crate::controller::v1alpha1::crd::claim::{ConfigMapClaim, SecretClaim};

pub async fn run(data: Data)  {
    join![
        startController::<ConfigMapClaim>(data.clone()),
        startController::<SecretClaim>(data.clone())
    ];

}