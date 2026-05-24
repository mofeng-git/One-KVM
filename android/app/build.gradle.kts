import org.gradle.api.tasks.Exec
import java.security.MessageDigest
import java.util.Properties

plugins {
    id("com.android.application")
}

val androidNdkVersion = "27.3.13750724"
val androidApiLevel = 21
val nativeCrateDir = layout.projectDirectory.dir("../native")
val rootCrateDir = layout.projectDirectory.dir("../..")
val nativeCargoOutputDir = layout.buildDirectory.dir("generated/oneKvm/cargoJniLibs")
val nativeOutputRoot = layout.buildDirectory.dir("generated/oneKvm/jniLibs")
val nativeAssetRoot = layout.buildDirectory.dir("generated/oneKvm/assets")
val defaultAndroidFfmpegRoot = rootProject.layout.projectDirectory.dir("../dist/android-ffmpeg-mediacodec")
val defaultAndroidLibyuvRoot = rootProject.layout.projectDirectory.dir("../dist/android-libyuv")
val defaultAndroidTurbojpegRoot = rootProject.layout.projectDirectory.dir("../dist/android-turbojpeg")
val defaultAndroidAlsaRoot = rootProject.layout.projectDirectory.dir("../dist/android-alsa")
val defaultAndroidOpusRoot = rootProject.layout.projectDirectory.dir("../dist/android-opus")
val androidFfmpegRoot = providers.environmentVariable("ONE_KVM_ANDROID_FFMPEG_ROOT")
    .orElse(defaultAndroidFfmpegRoot.asFile.absolutePath)
val androidLibyuvRoot = providers.environmentVariable("ONE_KVM_ANDROID_LIBYUV_ROOT")
    .orElse(defaultAndroidLibyuvRoot.asFile.absolutePath)
val androidTurbojpegRoot = providers.environmentVariable("ONE_KVM_ANDROID_TURBOJPEG_ROOT")
    .orElse(defaultAndroidTurbojpegRoot.asFile.absolutePath)
val androidAlsaRoot = providers.environmentVariable("ONE_KVM_ANDROID_ALSA_ROOT")
    .orElse(defaultAndroidAlsaRoot.asFile.absolutePath)
val androidOpusRoot = providers.environmentVariable("ONE_KVM_ANDROID_OPUS_ROOT")
    .orElse(defaultAndroidOpusRoot.asFile.absolutePath)
val selectedAndroidAbis = providers.environmentVariable("ONE_KVM_ANDROID_ABIS")
    .orElse("arm64-v8a,armeabi-v7a")
    .get()
    .split(',', ' ', ';')
    .map { it.trim() }
    .filter { it.isNotEmpty() }
    .distinct()
val androidBuildProfile = providers.environmentVariable("ONE_KVM_ANDROID_PROFILE")
    .orElse("debug")
    .get()
    .lowercase()
val oneKvmVersion = Regex("""(?m)^version\s*=\s*"([^"]+)"""")
    .find(rootCrateDir.file("Cargo.toml").asFile.readText())
    ?.groupValues
    ?.get(1)
    ?: throw GradleException("Failed to resolve version from root Cargo.toml")
val androidFfmpegSourceDir = rootProject.layout.projectDirectory
    .dir("../.tmp/android-ffmpeg-check/src/ffmpeg-rockchip")
val localProperties = Properties().apply {
    val file = rootProject.file("local.properties")
    if (file.exists()) {
        file.inputStream().use { load(it) }
    }
}
val androidSdkDir = file(
    providers.environmentVariable("ANDROID_HOME")
        .orElse(providers.environmentVariable("ANDROID_SDK_ROOT"))
        .orElse(localProperties.getProperty("sdk.dir") ?: "/root/android-sdk")
        .get(),
)
val androidNdkDir = androidSdkDir.resolve("ndk/$androidNdkVersion")

val androidFfmpegBuildScript = rootProject.layout.projectDirectory
    .dir("..")
    .file("scripts/build-android-ffmpeg-mediacodec.sh")
val androidLibyuvBuildScript = rootProject.layout.projectDirectory
    .dir("..")
    .file("scripts/build-android-libyuv.sh")
val androidTurbojpegBuildScript = rootProject.layout.projectDirectory
    .dir("..")
    .file("scripts/build-android-turbojpeg.sh")
val androidAlsaBuildScript = rootProject.layout.projectDirectory
    .dir("..")
    .file("scripts/build-android-alsa.sh")
val androidOpusBuildScript = rootProject.layout.projectDirectory
    .dir("..")
    .file("scripts/build-android-opus.sh")

val androidAbiTargets = mapOf(
    "arm64-v8a" to Triple("arm64", "aarch64-linux-android", "aarch64-linux-android"),
    "armeabi-v7a" to Triple("arm32", "armv7-linux-androideabi", "arm-linux-androideabi"),
)

val selectedAndroidAbiTargets = selectedAndroidAbis.associateWith { abi ->
    androidAbiTargets[abi] ?: throw GradleException(
        "Unsupported ONE_KVM_ANDROID_ABIS entry: $abi. Supported values: ${androidAbiTargets.keys.joinToString(", ")}",
    )
}

if (androidBuildProfile != "debug" && androidBuildProfile != "release") {
    throw GradleException("Unsupported ONE_KVM_ANDROID_PROFILE: $androidBuildProfile. Use debug or release.")
}

fun androidFfmpegBuildStamp(script: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
        .digest(script.readBytes())
        .joinToString("") { "%02x".format(it) }
    return "api=$androidApiLevel;abis=${selectedAndroidAbis.joinToString(",")};script=$digest"
}

fun androidFfmpegRequiredFiles(root: File): List<File> = listOf(
    "include/libavcodec/avcodec.h",
    "lib/libavcodec.a",
    "lib/libavutil.a",
).flatMap { path -> selectedAndroidAbis.map { abi -> root.resolve("$abi/$path") } }

fun androidLibyuvBuildStamp(script: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
        .digest(script.readBytes())
        .joinToString("") { "%02x".format(it) }
    val turbojpegScriptDigest = MessageDigest.getInstance("SHA-256")
        .digest(androidTurbojpegBuildScript.asFile.readBytes())
        .joinToString("") { "%02x".format(it) }
    return "api=$androidApiLevel;abis=${selectedAndroidAbis.joinToString(",")};script=$digest;turbojpegScript=$turbojpegScriptDigest"
}

fun androidLibyuvRequiredFiles(root: File): List<File> = listOf(
    "include/libyuv.h",
    "lib/libyuv.a",
).flatMap { path -> selectedAndroidAbis.map { abi -> root.resolve("$abi/$path") } }

fun androidTurbojpegBuildStamp(script: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
        .digest(script.readBytes())
        .joinToString("") { "%02x".format(it) }
    return "api=$androidApiLevel;abis=${selectedAndroidAbis.joinToString(",")};script=$digest"
}

fun androidAlsaBuildStamp(script: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
        .digest(script.readBytes())
        .joinToString("") { "%02x".format(it) }
    return "api=$androidApiLevel;abis=${selectedAndroidAbis.joinToString(",")};script=$digest"
}

fun androidOpusBuildStamp(script: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
        .digest(script.readBytes())
        .joinToString("") { "%02x".format(it) }
    return "api=$androidApiLevel;abis=${selectedAndroidAbis.joinToString(",")};script=$digest"
}

fun androidTurbojpegRequiredFiles(root: File): List<File> = listOf(
    "include/turbojpeg.h",
    "include/jpeglib.h",
    "lib/libjpeg.a",
    "lib/libturbojpeg.a",
).flatMap { path -> selectedAndroidAbis.map { abi -> root.resolve("$abi/$path") } }

fun androidAlsaRequiredFiles(root: File): List<File> = listOf(
    "include/alsa/asoundlib.h",
    "lib/libasound.so",
).flatMap { path -> selectedAndroidAbis.map { abi -> root.resolve("$abi/$path") } }

fun androidOpusRequiredFiles(root: File): List<File> = listOf(
    "include/opus/opus.h",
    "lib/libopus.so",
).flatMap { path -> selectedAndroidAbis.map { abi -> root.resolve("$abi/$path") } }

android {
    namespace = "cn.one_kvm.androidhost"
    compileSdk = 36
    ndkVersion = androidNdkVersion
    flavorDimensions += "abi"

    defaultConfig {
        applicationId = "cn.one_kvm.androidhost"
        minSdk = androidApiLevel
        targetSdk = 36
        versionCode = 1
        versionName = oneKvmVersion
    }

    productFlavors {
        create("arm32") {
            dimension = "abi"
            ndk {
                abiFilters += "armeabi-v7a"
            }
        }
        create("arm64") {
            dimension = "abi"
            ndk {
                abiFilters += "arm64-v8a"
            }
        }
    }

    sourceSets {
        getByName("main") {
            assets.directories.clear()
            jniLibs.directories.clear()
        }
        getByName("arm32") {
            assets.directories.add("build/generated/oneKvm/assets/arm32")
            jniLibs.directories.add("build/generated/oneKvm/jniLibs/arm32")
        }
        getByName("arm64") {
            assets.directories.add("build/generated/oneKvm/assets/arm64")
            jniLibs.directories.add("build/generated/oneKvm/jniLibs/arm64")
        }
    }
}

tasks.register<Exec>("buildAndroidFfmpegMediaCodec") {
    description = "Builds the default Android FFmpeg MediaCodec static libraries."
    group = "build"

    val ffmpegRoot = file(androidFfmpegRoot.get())
    val sourceDir = androidFfmpegSourceDir.asFile
    val scriptFile = androidFfmpegBuildScript.asFile
    val stampFile = ffmpegRoot.resolve(".one-kvm-android-ffmpeg.stamp")

    workingDir = rootProject.layout.projectDirectory.dir("..").asFile
    commandLine(
        "bash",
        scriptFile.absolutePath,
        "--source",
        sourceDir.absolutePath,
        "--output",
        ffmpegRoot.absolutePath,
        "--ndk",
        androidNdkDir.absolutePath,
        "--api",
        androidApiLevel.toString(),
        "--abis",
        selectedAndroidAbis.joinToString(","),
    )

    inputs.dir(sourceDir)
    inputs.file(scriptFile)
    outputs.dir(ffmpegRoot)

    onlyIf {
        val hasAndroidFfmpeg = androidFfmpegRequiredFiles(ffmpegRoot).all { it.exists() }
        val hasCurrentBuildStamp =
            stampFile.exists() && stampFile.readText() == androidFfmpegBuildStamp(scriptFile)
        if (!hasAndroidFfmpeg && !sourceDir.resolve("configure").exists()) {
            throw GradleException(
                "Missing Android FFmpeg MediaCodec build at ${ffmpegRoot.absolutePath}, " +
                    "and source was not found at ${sourceDir.absolutePath}",
            )
        }
        !hasAndroidFfmpeg || !hasCurrentBuildStamp
    }

    doLast {
        stampFile.writeText(androidFfmpegBuildStamp(scriptFile))
    }
}

tasks.register<Exec>("buildAndroidLibyuv") {
    description = "Builds Android libyuv static libraries."
    group = "build"

    val libyuvRoot = file(androidLibyuvRoot.get())
    val turbojpegRoot = file(androidTurbojpegRoot.get())
    val scriptFile = androidLibyuvBuildScript.asFile
    val stampFile = libyuvRoot.resolve(".one-kvm-android-libyuv.stamp")

    dependsOn("buildAndroidTurbojpeg")

    workingDir = rootProject.layout.projectDirectory.dir("..").asFile
    commandLine(
        "bash",
        scriptFile.absolutePath,
        "--output",
        libyuvRoot.absolutePath,
        "--ndk",
        androidNdkDir.absolutePath,
        "--api",
        androidApiLevel.toString(),
        "--abis",
        selectedAndroidAbis.joinToString(","),
        "--jpeg-root",
        turbojpegRoot.absolutePath,
    )

    inputs.file(scriptFile)
    outputs.dir(libyuvRoot)

    onlyIf {
        val hasAndroidLibyuv = androidLibyuvRequiredFiles(libyuvRoot).all { it.exists() }
        val hasCurrentBuildStamp =
            stampFile.exists() && stampFile.readText() == androidLibyuvBuildStamp(scriptFile)
        !hasAndroidLibyuv || !hasCurrentBuildStamp
    }

    doLast {
        stampFile.writeText(androidLibyuvBuildStamp(scriptFile))
    }
}

tasks.register<Exec>("buildAndroidTurbojpeg") {
    description = "Builds Android TurboJPEG static libraries."
    group = "build"

    val turbojpegRoot = file(androidTurbojpegRoot.get())
    val scriptFile = androidTurbojpegBuildScript.asFile
    val stampFile = turbojpegRoot.resolve(".one-kvm-android-turbojpeg.stamp")

    workingDir = rootProject.layout.projectDirectory.dir("..").asFile
    commandLine(
        "bash",
        scriptFile.absolutePath,
        "--output",
        turbojpegRoot.absolutePath,
        "--ndk",
        androidNdkDir.absolutePath,
        "--api",
        androidApiLevel.toString(),
        "--abis",
        selectedAndroidAbis.joinToString(","),
    )

    inputs.file(scriptFile)
    outputs.dir(turbojpegRoot)

    onlyIf {
        val hasAndroidTurbojpeg = androidTurbojpegRequiredFiles(turbojpegRoot).all { it.exists() }
        val hasCurrentBuildStamp =
            stampFile.exists() && stampFile.readText() == androidTurbojpegBuildStamp(scriptFile)
        !hasAndroidTurbojpeg || !hasCurrentBuildStamp
    }

    doLast {
        stampFile.writeText(androidTurbojpegBuildStamp(scriptFile))
    }
}

tasks.register<Exec>("buildAndroidAlsa") {
    description = "Builds Android ALSA shared libraries."
    group = "build"

    val alsaRoot = file(androidAlsaRoot.get())
    val scriptFile = androidAlsaBuildScript.asFile
    val stampFile = alsaRoot.resolve(".one-kvm-android-alsa.stamp")

    workingDir = rootProject.layout.projectDirectory.dir("..").asFile
    commandLine(
        "bash",
        scriptFile.absolutePath,
        "--output",
        alsaRoot.absolutePath,
        "--ndk",
        androidNdkDir.absolutePath,
        "--api",
        androidApiLevel.toString(),
        "--abis",
        selectedAndroidAbis.joinToString(","),
    )

    inputs.file(scriptFile)
    outputs.dir(alsaRoot)

    onlyIf {
        val hasAndroidAlsa = androidAlsaRequiredFiles(alsaRoot).all { it.exists() }
        val hasCurrentBuildStamp =
            stampFile.exists() && stampFile.readText() == androidAlsaBuildStamp(scriptFile)
        !hasAndroidAlsa || !hasCurrentBuildStamp
    }

    doLast {
        stampFile.writeText(androidAlsaBuildStamp(scriptFile))
    }
}

tasks.register<Exec>("buildAndroidOpus") {
    description = "Builds Android Opus shared libraries."
    group = "build"

    val opusRoot = file(androidOpusRoot.get())
    val scriptFile = androidOpusBuildScript.asFile
    val stampFile = opusRoot.resolve(".one-kvm-android-opus.stamp")

    workingDir = rootProject.layout.projectDirectory.dir("..").asFile
    commandLine(
        "bash",
        scriptFile.absolutePath,
        "--output",
        opusRoot.absolutePath,
        "--ndk",
        androidNdkDir.absolutePath,
        "--api",
        androidApiLevel.toString(),
        "--abis",
        selectedAndroidAbis.joinToString(","),
    )

    inputs.file(scriptFile)
    outputs.dir(opusRoot)

    onlyIf {
        val hasAndroidOpus = androidOpusRequiredFiles(opusRoot).all { it.exists() }
        val hasCurrentBuildStamp =
            stampFile.exists() && stampFile.readText() == androidOpusBuildStamp(scriptFile)
        !hasAndroidOpus || !hasCurrentBuildStamp
    }

    doLast {
        stampFile.writeText(androidOpusBuildStamp(scriptFile))
    }
}

val cargoBuildAndroidAbiTaskNames = selectedAndroidAbiTargets.map { (abi, targets) ->
    val (flavor, _, _) = targets
    val taskName = "cargoBuildAndroid" + flavor.replaceFirstChar {
        if (it.isLowerCase()) it.titlecase() else it.toString()
    }

    tasks.register<Exec>(taskName) {
        description = "Builds the Android Rust bootstrap libraries for $abi."
        group = "build"

        dependsOn(
            "buildAndroidFfmpegMediaCodec",
            "buildAndroidLibyuv",
            "buildAndroidTurbojpeg",
            "buildAndroidAlsa",
            "buildAndroidOpus",
        )

        val cargoCommand = mutableListOf(
            "cargo",
            "ndk",
            "-t",
            abi,
            "-P",
            androidApiLevel.toString(),
            "-o",
            nativeCargoOutputDir.get().asFile.absolutePath,
            "build",
            "--lib",
            "--bins",
        )
        if (androidBuildProfile == "release") {
            cargoCommand.add("--release")
        }

        workingDir = nativeCrateDir.asFile
        commandLine(cargoCommand)
        args("--features", "android-mediacodec")
        environment("ONE_KVM_ANDROID_FFMPEG_ROOT", androidFfmpegRoot.get())
        environment("ONE_KVM_ANDROID_LIBYUV_ROOT", androidLibyuvRoot.get())
        environment("ONE_KVM_ANDROID_LIBYUV_STATIC", "1")
        environment("TURBOJPEG_SOURCE", "explicit")
        environment("TURBOJPEG_STATIC", "1")
        environment(
            "TURBOJPEG_LIB_DIR",
            file(androidTurbojpegRoot.get()).resolve("$abi/lib").absolutePath,
        )
        environment(
            "TURBOJPEG_INCLUDE_DIR",
            file(androidTurbojpegRoot.get()).resolve("$abi/include").absolutePath,
        )
        environment("PKG_CONFIG_ALLOW_CROSS", "1")
        environment(
            "PKG_CONFIG_LIBDIR",
            file(androidAlsaRoot.get()).resolve("$abi/lib/pkgconfig").absolutePath,
        )
        environment("PKG_CONFIG_SYSROOT_DIR", "")
        environment("LIBOPUS_NO_PKG", "1")
        environment("LIBOPUS_LIB_DIR", file(androidOpusRoot.get()).resolve("$abi/lib").absolutePath)
        environment("ANDROID_HOME", androidSdkDir.absolutePath)
        environment("ANDROID_SDK_ROOT", androidSdkDir.absolutePath)
        environment("ANDROID_NDK_HOME", androidNdkDir.absolutePath)
        environment("ANDROID_NDK", androidNdkDir.absolutePath)
        environment("ANDROID_NDK_ROOT", androidNdkDir.absolutePath)

        inputs.files(
            nativeCrateDir.file("Cargo.toml"),
            nativeCrateDir.dir("src"),
            rootCrateDir.file("Cargo.lock"),
            rootCrateDir.file("Cargo.toml"),
            rootCrateDir.file("build.rs"),
            rootCrateDir.dir("libs"),
            rootCrateDir.dir("res/vcpkg/libyuv"),
            rootCrateDir.dir("src"),
        )
        outputs.dir(nativeCargoOutputDir)
        outputs.dir(file(androidFfmpegRoot.get()))
        outputs.dir(file(androidLibyuvRoot.get()))
        outputs.dir(file(androidTurbojpegRoot.get()))
        outputs.dir(file(androidAlsaRoot.get()))
        outputs.dir(file(androidOpusRoot.get()))
    }

    taskName
}

tasks.register("cargoBuildAndroid") {
    description = "Builds the Android Rust bootstrap libraries."
    group = "build"

    dependsOn(cargoBuildAndroidAbiTaskNames)

    outputs.dir(nativeOutputRoot)
    outputs.dir(nativeAssetRoot)

    doLast {
        selectedAndroidAbiTargets.forEach { (abi, targets) ->
            val (flavor, rustTriple, ndkTriple) = targets
            val nativeLibSource = nativeCargoOutputDir.get().file("$abi/libone_kvm_android_bootstrap.so").asFile
            if (!nativeLibSource.exists()) {
                throw GradleException("Missing Android JNI library: ${nativeLibSource.absolutePath}")
            }
            copy {
                from(nativeLibSource)
                into(nativeOutputRoot.get().dir(flavor).dir(abi))
            }

            val source = nativeCrateDir.file("target/$rustTriple/$androidBuildProfile/one-kvm-android-host").asFile
            if (!source.exists()) {
                throw GradleException("Missing Android host binary: ${source.absolutePath}")
            }
            copy {
                from(source)
                into(nativeAssetRoot.get().dir(flavor).dir("bin/$abi"))
                rename { "one-kvm-android-host" }
            }

            val cxxShared = androidNdkDir
                .resolve("toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/$ndkTriple/libc++_shared.so")
            if (!cxxShared.exists()) {
                throw GradleException("Missing NDK libc++_shared.so: ${cxxShared.absolutePath}")
            }
            copy {
                from(cxxShared)
                into(nativeOutputRoot.get().dir(flavor).dir(abi))
            }
            copy {
                from(cxxShared)
                into(nativeAssetRoot.get().dir(flavor).dir("bin/$abi"))
            }

        val alsaShared = file(androidAlsaRoot.get()).resolve("$abi/lib/libasound.so")
        if (!alsaShared.exists()) {
            throw GradleException("Missing Android ALSA library: ${alsaShared.absolutePath}")
        }
        copy {
            from(alsaShared)
            into(nativeOutputRoot.get().dir(flavor).dir(abi))
        }
        copy {
            from(alsaShared)
            into(nativeAssetRoot.get().dir(flavor).dir("bin/$abi"))
        }
        copy {
            from(file(androidAlsaRoot.get()).resolve("$abi/share/alsa"))
            into(nativeAssetRoot.get().dir(flavor).dir("bin/$abi/alsa"))
        }

        val opusShared = file(androidOpusRoot.get()).resolve("$abi/lib/libopus.so")
        if (!opusShared.exists()) {
            throw GradleException("Missing Android Opus library: ${opusShared.absolutePath}")
        }
        copy {
            from(opusShared)
            into(nativeOutputRoot.get().dir(flavor).dir(abi))
        }
            copy {
                from(opusShared)
                into(nativeAssetRoot.get().dir(flavor).dir("bin/$abi"))
            }
        }
    }
}

tasks.named("preBuild") {
    dependsOn("cargoBuildAndroid")
}
