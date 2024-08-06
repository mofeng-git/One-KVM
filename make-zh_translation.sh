pybabel extract -F babel.cfg -o message.pot .
pybabel update -d kvmd/i18n -l zh -D message -i message.pot
pybabel compile -i kvmd/i18n/zh/LC_MESSAGES/message.po -o kvmd/i18n/zh/LC_MESSAGES/message.mo