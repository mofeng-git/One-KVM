use jni::errors::{ErrorPolicy, ThrowRuntimeExAndDefault};
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jint, jstring};
use jni::{Env, EnvOutcome, EnvUnowned};
use one_kvm::runtime::android::{self, AndroidRuntimeConfig};

#[derive(Debug)]
struct BridgeError(String);

impl From<jni::errors::Error> for BridgeError {
    fn from(err: jni::errors::Error) -> Self {
        Self(err.to_string())
    }
}

impl From<String> for BridgeError {
    fn from(err: String) -> Self {
        Self(err)
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Default)]
struct StatusPolicy;

impl ErrorPolicy<jint, BridgeError> for StatusPolicy {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = ();

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        _env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        _err: BridgeError,
    ) -> jni::errors::Result<jint> {
        Ok(-1)
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        _env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        _payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<jint> {
        Ok(-1)
    }
}

#[derive(Debug, Default)]
struct StringResultPolicy;

impl ErrorPolicy<String, BridgeError> for StringResultPolicy {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = ();

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        _env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        err: BridgeError,
    ) -> jni::errors::Result<String> {
        Ok(format!("start failed: {err}"))
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        _env: &mut Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        _payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<String> {
        Ok("start failed: panic in native bridge".to_string())
    }
}

#[no_mangle]
pub extern "system" fn Java_cn_one_1kvm_androidhost_NativeBridge_setEnv<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
    value: JString<'local>,
) -> jint {
    let outcome: EnvOutcome<'local, jint, BridgeError> = env.with_env_no_catch(|env| {
        let name = name
            .try_to_string(env)
            .map_err(|err| BridgeError(format!("invalid env name: {err}")))?;
        let value = value
            .try_to_string(env)
            .map_err(|err| BridgeError(format!("invalid env value: {err}")))?;
        if name.contains('\0') || value.contains('\0') {
            return Err(BridgeError("env contains NUL".to_string()));
        }
        std::env::set_var(name, value);
        Ok(0)
    });

    outcome.resolve_with::<StatusPolicy, _>(|| ())
}

#[no_mangle]
pub extern "system" fn Java_cn_one_1kvm_androidhost_NativeBridge_initTlsVerifier<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) -> jint {
    let outcome: EnvOutcome<'local, jint, BridgeError> =
        env.with_env_no_catch(|env| init_tls_verifier(env, context));

    outcome.resolve_with::<StatusPolicy, _>(|| ())
}

#[cfg(target_os = "android")]
fn init_tls_verifier(env: &mut Env<'_>, context: JObject<'_>) -> Result<jint, BridgeError> {
    rustls_platform_verifier::android::init_with_env(env, context)
        .map_err(|err| BridgeError(format!("failed to initialize rustls platform verifier: {err}")))?;
    Ok(0)
}

#[cfg(not(target_os = "android"))]
fn init_tls_verifier(_env: &mut Env<'_>, _context: JObject<'_>) -> Result<jint, BridgeError> {
    Ok(0)
}

#[no_mangle]
pub extern "system" fn Java_cn_one_1kvm_androidhost_NativeBridge_startHost<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    data_dir: JString<'local>,
    bind_address: JString<'local>,
    port: i32,
) -> jstring {
    let outcome: EnvOutcome<'local, String, BridgeError> = env.with_env_no_catch(|env| {
        let data_dir = data_dir
            .try_to_string(env)
            .map_err(|err| BridgeError(format!("invalid data dir: {err}")))?;
        let bind_address = bind_address
            .try_to_string(env)
            .map_err(|err| BridgeError(format!("invalid bind address: {err}")))?;
        let port = u16::try_from(port).map_err(|_| BridgeError("invalid port".to_string()))?;

        android::start(AndroidRuntimeConfig {
            data_dir,
            bind_address,
            port,
        })
        .map_err(BridgeError)
    });

    let result = outcome.resolve_with::<StringResultPolicy, _>(|| ());

    env.with_env_no_catch(|env| env.new_string(result))
        .resolve_with::<ThrowRuntimeExAndDefault, _>(|| ())
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_cn_one_1kvm_androidhost_NativeBridge_stopHost<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    env.with_env_no_catch(|env| env.new_string(android::stop()))
        .resolve_with::<ThrowRuntimeExAndDefault, _>(|| ())
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_cn_one_1kvm_androidhost_NativeBridge_hostStatus<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    env.with_env_no_catch(|env| env.new_string(android::status()))
        .resolve_with::<ThrowRuntimeExAndDefault, _>(|| ())
        .into_raw()
}

#[no_mangle]
pub extern "system" fn Java_cn_one_1kvm_androidhost_NativeBridge_kernelVersion<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jstring {
    env.with_env_no_catch(|env| env.new_string(env!("CARGO_PKG_VERSION")))
        .resolve_with::<ThrowRuntimeExAndDefault, _>(|| ())
        .into_raw()
}
