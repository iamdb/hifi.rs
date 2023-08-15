#[macro_use]
pub mod db;

#[macro_export]
macro_rules! acquire {
    ($self:ident) => {
        $self.pool.acquire().await
    };
}

#[macro_export]
macro_rules! query {
    ($query:expr, $conn:ident, $value:ident) => {
        sqlx::query!($query, $value)
            .execute(&mut *$conn)
            .await
            .expect("database failure")
    };
}

#[macro_export]
macro_rules! get_all {
    ($query:ident, $return_type:ident, $conn:ident) => {
        sqlx::query_as!($return_type, $query)
            .fetch_all(&mut $conn)
            .await
            .expect("database failure")
    };
}

#[macro_export]
macro_rules! get_one {
    ($query:expr, $return_type:ident, $conn:ident) => {
        sqlx::query_as!($return_type, $query)
            .fetch_one(&mut *$conn)
            .await
    };
}
