/// Contains a queue implementation using Redis.
pub mod redis_queue;

use serde_json;
use redis;
use error::*;
use config::Config;
use std::marker::Sized;

/// Trait that represents a backend that can be used to store jobs.
pub trait JobQueue
where
    Self: Sized,
{
    /// The type required to configure the queue.
    type Config;

    /// Create a new queue with the given config.
    fn new(init: &Self::Config) -> RobinResult<Self>;

    /// Push a job into the queue.
    fn enqueue(&self, enq_job: EnqueuedJob, iden: QueueIdentifier) -> RobinResult<()>;

    /// Pull a job from the queue.
    fn dequeue(&self, iden: QueueIdentifier) -> Result<EnqueuedJob, NoJobDequeued>;

    /// Delete all jobs from the queue.
    fn delete_all(&self, iden: QueueIdentifier) -> RobinResult<()>;

    /// Get the number of jobs in the queue.
    fn size(&self, iden: QueueIdentifier) -> RobinResult<usize>;
}

/// The number of times a job has been retried, if ever.
#[derive(Deserialize, Serialize, Debug, Copy, Clone)]
pub enum RetryCount {
    /// The job has never been retried,
    NeverRetried,

    /// The job has retried given number of times.
    Count(u32),
}

impl RetryCount {
    /// Increment the retry counter by one
    pub fn increment(&self) -> RetryCount {
        match *self {
            RetryCount::NeverRetried => RetryCount::Count(1),
            RetryCount::Count(n) => RetryCount::Count(n + 1),
        }
    }

    /// `true` if the retry limit in the config has been reached, `false` otherwise
    pub fn limit_reached(&self, config: &Config) -> bool {
        match *self {
            RetryCount::NeverRetried => false,
            RetryCount::Count(n) => n > config.retry_count_limit,
        }
    }
}

/// The data structure that gets serialized and put into Redis.
#[derive(Deserialize, Serialize, Debug, Builder)]
pub struct EnqueuedJob {
    name: String,
    args: String,
    retry_count: RetryCount,
}

impl EnqueuedJob {
    /// Get the name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the arguments
    pub fn args(&self) -> &str {
        &self.args
    }

    /// Get the retry count
    pub fn retry_count(&self) -> &RetryCount {
        &self.retry_count
    }
}

/// Reasons why attempting to dequeue a job didn't yield a job.
#[derive(Debug)]
pub enum NoJobDequeued {
    /// The timeout was hit. This will most likely retry dequeueing a job
    BecauseTimeout,

    /// Because there some error.
    BecauseError(Error),
}

impl From<redis::RedisError> for NoJobDequeued {
    fn from(error: redis::RedisError) -> NoJobDequeued {
        NoJobDequeued::BecauseError(Error::from(error))
    }
}

impl From<serde_json::Error> for NoJobDequeued {
    fn from(error: serde_json::Error) -> NoJobDequeued {
        NoJobDequeued::BecauseError(Error::from(error))
    }
}

impl From<Error> for NoJobDequeued {
    fn from(error: Error) -> NoJobDequeued {
        NoJobDequeued::BecauseError(error)
    }
}

/// The different queues supported by Robin.
#[derive(EachVariant, Debug, Copy, Clone)]
pub enum QueueIdentifier {
    /// The main queue all new jobs are put into.
    Main,

    /// If a job from the main queue fails it gets put into the retry queue
    /// and retried later.
    Retry,
}

impl QueueIdentifier {
    /// Convert the name to the string used for the Redis key.
    pub fn redis_queue_name(&self) -> String {
        match *self {
            QueueIdentifier::Main => "main".to_string(),
            QueueIdentifier::Retry => "retry".to_string(),
        }
    }
}
