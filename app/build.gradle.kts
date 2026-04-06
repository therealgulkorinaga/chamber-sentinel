plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.chamber.sentinel"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.chamber.sentinel"
        minSdk = 31
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.12.0")
    implementation("androidx.appcompat:appcompat:1.6.1")
    implementation("com.google.android.material:material:1.11.0")
    implementation("androidx.constraintlayout:constraintlayout:2.1.4")
    implementation("androidx.fragment:fragment-ktx:1.6.2")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.7.0")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.7.0")
    implementation("androidx.recyclerview:recyclerview:1.3.2")
    implementation("androidx.camera:camera-camera2:1.3.1")
}

// ---------------------------------------------------------------------------
// Rust / Cargo NDK build integration
// ---------------------------------------------------------------------------

val rustDir = rootProject.projectDir.resolve("rust")
val ndkTarget = "aarch64-linux-android"
val jniLibsDir = projectDir.resolve("src/main/jniLibs/arm64-v8a")

tasks.register<Exec>("cargoBuild") {
    description = "Build Rust cdylib for Android via cargo"
    workingDir = rustDir

    val ndkHome = android.ndkDirectory.absolutePath
    val toolchain = "$ndkHome/toolchains/llvm/prebuilt/${osTag()}/bin"
    val apiLevel = android.defaultConfig.minSdk!!

    environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", "$toolchain/aarch64-linux-android${apiLevel}-clang")
    environment("CC_aarch64_linux_android", "$toolchain/aarch64-linux-android${apiLevel}-clang")
    environment("AR_aarch64_linux_android", "$toolchain/llvm-ar")

    commandLine("cargo", "build", "--target", ndkTarget, "--release", "-p", "chamber-core")

    doLast {
        jniLibsDir.mkdirs()
        val soFile = rustDir.resolve("target/$ndkTarget/release/libchamber_core.so")
        if (soFile.exists()) {
            soFile.copyTo(jniLibsDir.resolve("libchamber_core.so"), overwrite = true)
        }
    }
}

tasks.register<Exec>("cargoClean") {
    description = "Run cargo clean in the Rust workspace"
    workingDir = rustDir
    commandLine("cargo", "clean")
}

tasks.named("preBuild") {
    dependsOn("cargoBuild")
}

fun osTag(): String {
    val os = System.getProperty("os.name").lowercase()
    return when {
        os.contains("mac") -> "darwin-x86_64"
        os.contains("linux") -> "linux-x86_64"
        os.contains("win") -> "windows-x86_64"
        else -> "linux-x86_64"
    }
}
