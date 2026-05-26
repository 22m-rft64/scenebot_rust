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
                withCredentials([file(credentialsId: 'scenebot-env', variable: 'ENVFILE')]) {
                    sh '''
                        set -a
                        . "$ENVFILE"
                        set +a
                        docker run --rm \
                            --network shared-mongodb-net \
                            -e DISCORD_BOT_TOKEN="$IREM_DISCORD_BOT_TOKEN" \
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
                withCredentials([file(credentialsId: 'scenebot-env', variable: 'ENVFILE')]) {
                    sh '''
                        set -a
                        . "$ENVFILE"
                        set +a
                        docker run --rm \
                            --network shared-mongodb-net \
                            -e DISCORD_BOT_TOKEN="$SCENE_DISCORD_BOT_TOKEN" \
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
                withCredentials([file(credentialsId: 'scenebot-env', variable: 'ENVFILE')]) {
                    sh '''
                        docker compose --env-file "$ENVFILE" down --remove-orphans || true
                        docker compose --env-file "$ENVFILE" up -d
                    '''
                }
            }
        }
    }

    post {
        always {
            sh 'docker image prune -f --filter "until=168h" || true'
        }
    }
}
