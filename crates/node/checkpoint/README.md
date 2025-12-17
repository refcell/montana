# montana-checkpoint

Checkpoint persistence for Montana node resumption.

## Overview

This crate provides the `Checkpoint` type which tracks node progress and enables resumption after restart. It prevents duplicate batch submissions by tracking the last successfully submitted batch number.

## License

Licensed under the MIT license.
