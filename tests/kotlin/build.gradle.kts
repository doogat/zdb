plugins {
    kotlin("jvm") version "2.1.0"
}

repositories {
    mavenCentral()
}

dependencies {
    // Generated UniFFI Kotlin bindings — copy from out/kotlin/ after AAR build
    // implementation(files("../../out/kotlin/zetteldb.aar"))

    // JNA required by UniFFI-generated Kotlin code
    implementation("net.java.dev.jna:jna:5.16.0")

    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.4")
}

tasks.test {
    useJUnitPlatform()
}

// Point JNA to the native library built for the host platform
tasks.test {
    systemProperty("jna.library.path", "../../target/release")
}
