pub trait GetConfig<T> {
    fn get_config(&self) -> &T;
}
