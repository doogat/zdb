plugins {
    kotlin("jvm") version "2.3.10"
}

repositories {
    mavenCentral()
}

// JDK 25 bytecode target not yet supported — use 23
tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_23)
    }
}

tasks.withType<JavaCompile> {
    sourceCompatibility = "23"
    targetCompatibility = "23"
}

dependencies {
    // JNA required by UniFFI-generated Kotlin code
    implementation("net.java.dev.jna:jna:5.16.0")

    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.4")
}

tasks.test {
    useJUnitPlatform()
    // Point JNA to the native library built for the host platform
    systemProperty("jna.library.path", rootProject.projectDir.resolve("../../target/release").absolutePath)
    testLogging {
        showStandardStreams = true
    }
}
