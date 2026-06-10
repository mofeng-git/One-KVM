pluginManagement {
    fun isEnabled(value: String?): Boolean = when (value?.lowercase()) {
        "1", "true", "yes", "on" -> true
        else -> false
    }
    val mirrorAcceleration = isEnabled(System.getenv("CHINAMIRRO"))

    repositories {
        if (mirrorAcceleration) {
            maven("https://maven.aliyun.com/repository/google")
            maven("https://maven.aliyun.com/repository/public")
            maven("https://maven.aliyun.com/repository/gradle-plugin")
        }
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    fun isEnabled(value: String?): Boolean = when (value?.lowercase()) {
        "1", "true", "yes", "on" -> true
        else -> false
    }
    val mirrorAcceleration = isEnabled(System.getenv("CHINAMIRRO"))

    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        if (mirrorAcceleration) {
            maven("https://maven.aliyun.com/repository/google")
            maven("https://maven.aliyun.com/repository/public")
        }
        google()
        mavenCentral()
    }
}

rootProject.name = "OneKvmAndroidHost"
include(":app")
