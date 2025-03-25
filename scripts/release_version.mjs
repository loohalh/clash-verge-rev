import { execSync } from "child_process";
import fs from "fs";
import path from "path";

// Function to update version in package.json
function updatePackageVersion(version) {
  try {
    const packageJsonPath = path.resolve(process.cwd(), "package.json");

    // Read package.json
    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));

    // Store original version
    const originalVersion = packageJson.version;

    // Update version
    packageJson.version = version;

    // Write back to package.json with proper formatting
    fs.writeFileSync(
      packageJsonPath,
      JSON.stringify(packageJson, null, 2) + "\n",
    );

    console.log(`Version updated from ${originalVersion} to ${version}`);

    return true;
  } catch (error) {
    console.error(`Failed to update package version: ${error.message}`);
    return false;
  }
}

// Main function to handle the release process
function releaseVersion() {
  try {
    const newVersion = process.argv[2];

    if (!newVersion) {
      console.error("Please provide a version number as an argument");
      process.exit(1);
    }

    console.log(`Starting release process for version ${newVersion}...`);

    // Update version in package.json
    if (!updatePackageVersion(newVersion)) {
      process.exit(1);
    }

    // Git operations
    console.log("Committing changes...");
    execSync("git add package.json", { stdio: "inherit" });
    execSync(`git commit -m "chore: release version ${newVersion}"`, {
      stdio: "inherit",
    });

    // Create tag
    console.log(`Creating tag v${newVersion}...`);
    execSync(`git tag v${newVersion}`, { stdio: "inherit" });

    console.log("Pushing changes and tags...");
    execSync("git push", { stdio: "inherit" });
    execSync("git push --tags", { stdio: "inherit" });

    console.log(`Version ${newVersion} released successfully!`);
  } catch (error) {
    console.error(`Release process failed: ${error.message}`);
    process.exit(1);
  }
}

releaseVersion();
