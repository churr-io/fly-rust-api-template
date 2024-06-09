#!/bin/bash
set -x
APP_ENV="${APP_ENV:-beta}"

if [ "$APP_ENV" == "prod" ]
then
  	APP=cookbook-api
else
    APP=cookbook-api-$APP_ENV
fi

flyctl deploy -e "APP_ENV=$APP_ENV" --app "$APP" "$@"
