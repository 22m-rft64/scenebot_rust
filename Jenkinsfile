pipeline {
    agent any

    options {
        timestamps()
        ansiColor('xterm')
        buildDiscarder(logRotator(numToKeepStr: '20'))
        disableConcurrentBuilds()
    }

    triggers {
        githubPush()
    }

    environment {
        IMAGE_LATEST = "scene_rust:latest"
        IMAGE_TAG    = "scene_rust:${env.BUILD_NUMBER}"
    }

    stages {
        stage('Skip non-main') {
            when {
                not { branch 'main' }
            }
            steps {
                echo "Branch ${env.BRANCH_NAME} — build only, no deploy."
            }
        }

        stage('Build image') {
            steps {
                sh '''
                    docker build \
                        -t "$IMAGE_TAG" \
                        -t "$IMAGE_LATEST" \
                        .
                '''
            }
        }

        stage('Migrate (irem)') {
            when { branch 'main' }
            steps {
                withCredentials([
                    string(credentialsId: 'irem-discord-bot-token', variable: 'TOKEN'),
                    string(credentialsId: 'mongodb-uri',            variable: 'DB_URI')
                ]) {
                    sh '''
                        docker run --rm \
                            --network shared-mongodb-net \
                            -e DISCORD_BOT_TOKEN="$TOKEN" \
                            -e DB_URI="$DB_URI" \
                            -e BOT_DB_NAME=trainingsuit_mari_guilds \
                            -e RUST_LOG=info \
                            "$IMAGE_LATEST" \
                            /app/migrate_guild_ids
                    '''
                }
            }
        }

        stage('Migrate (scene)') {
            when { branch 'main' }
            steps {
                withCredentials([
                    string(credentialsId: 'scene-discord-bot-token', variable: 'TOKEN'),
                    string(credentialsId: 'mongodb-uri',             variable: 'DB_URI')
                ]) {
                    sh '''
                        docker run --rm \
                            --network shared-mongodb-net \
                            -e DISCORD_BOT_TOKEN="$TOKEN" \
                            -e DB_URI="$DB_URI" \
                            -e BOT_DB_NAME=scene_guilds \
                            -e RUST_LOG=info \
                            "$IMAGE_LATEST" \
                            /app/migrate_guild_ids
                    '''
                }
            }
        }

        stage('Deploy') {
            when { branch 'main' }
            steps {
                withCredentials([
                    string(credentialsId: 'irem-discord-bot-token',  variable: 'IREM_DISCORD_BOT_TOKEN'),
                    string(credentialsId: 'scene-discord-bot-token', variable: 'SCENE_DISCORD_BOT_TOKEN'),
                    string(credentialsId: 'mongodb-uri',             variable: 'DB_URI')
                ]) {
                    sh '''
                        docker compose down --remove-orphans || true
                        docker compose up -d
                    '''
                }
            }
        }
    }

    post {
        always {
            // dangling 이미지 정리. labels=scenebot=true는 다음 빌드에서 prune 대상.
            sh 'docker image prune -f --filter "until=168h" || true'
        }
    }
}
