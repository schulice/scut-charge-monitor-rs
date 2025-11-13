# SCUT-Charge-Monitor-Rs

This is a reimplement for [scut-charge-monitor](https://github.com/c-w-xiaohei/scut-charge-monitor) in rust, with the aim to run it on embedding device like router.

## Usage

You may find binary on github release by github action, download it and prepare a env/json config file.

The template is on `.env.template`, you can alse use a json file with `snake_case` key.

For electricity usage report, now only support [ftqq](https://sct.ftqq.com/sendkey). Later may support smtp.
